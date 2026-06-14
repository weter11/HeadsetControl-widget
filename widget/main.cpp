#include "widget_logic.hpp"

#include <QAction>
#include <QActionGroup>
#include <QApplication>
#include <QCoreApplication>
#include <QCursor>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QIcon>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QJsonValue>
#include <QMenu>
#include <QProcess>
#include <QSocketNotifier>
#include <QStandardPaths>
#include <QSystemTrayIcon>
#include <QTimer>

#include <libudev.h>

#include <algorithm>
#include <array>
#include <cstdint>
#include <optional>

namespace {

using headsetcontrol::widget::CHARGE_THRESHOLD_OPTIONS;
using headsetcontrol::widget::DISCHARGE_THRESHOLD_OPTIONS;
using headsetcontrol::widget::INACTIVE_TIME_OPTIONS;
using headsetcontrol::widget::PowerState;
using headsetcontrol::widget::SIDETONE_LEVELS;
using headsetcontrol::widget::contains;
using headsetcontrol::widget::evaluateNotificationThresholds;

constexpr int REFRESH_INTERVAL_MS = 15000;
constexpr int COMMAND_TIMEOUT_MS  = 8000;

struct WidgetConfig {
    int discharge_threshold         = 20;
    int charge_threshold            = 80;
    bool has_sidetone_preference    = false;
    int sidetone_level              = SIDETONE_LEVELS[0];
    bool has_inactive_preference    = false;
    int inactive_time_minutes       = INACTIVE_TIME_OPTIONS[0];
};

struct DeviceState {
    bool connected                  = false;
    QString device_name             = QObject::tr("Headset");
    std::optional<int> battery_level;
    PowerState power_state          = PowerState::Unknown;
    uint16_t vendor_id              = 0;
    uint16_t product_id             = 0;
};

struct CommandResult {
    bool ok             = false;
    bool timed_out      = false;
    int exit_code       = -1;
    QString stdout_text;
    QString stderr_text;
};

[[nodiscard]] QString powerStateLabel(PowerState state)
{
    switch (state) {
    case PowerState::Charging:
        return QObject::tr("Charging");
    case PowerState::Discharging:
        return QObject::tr("Discharging");
    case PowerState::Unavailable:
        return QObject::tr("Unavailable");
    default:
        return QObject::tr("Unknown");
    }
}

[[nodiscard]] QString batterySummary(const DeviceState& state)
{
    if (!state.connected) {
        return QObject::tr("Battery: --");
    }

    if (!state.battery_level.has_value()) {
        return QObject::tr("Battery: unavailable");
    }

    return QObject::tr("Battery: %1%").arg(*state.battery_level);
}

class HeadsetControlWidgetApp {
public:
    HeadsetControlWidgetApp()
        : config_path_(QDir(QStandardPaths::writableLocation(QStandardPaths::ConfigLocation)).filePath("headsetcontrol-widget/config.json"))
    {
        loadConfig();
        setupTray();
        setupMenu();
        setupUdevMonitor();

        poll_timer_.setInterval(REFRESH_INTERVAL_MS);
        QObject::connect(&poll_timer_, &QTimer::timeout, [&]() { refreshState(); });
        poll_timer_.start();

        refreshState();
    }

    ~HeadsetControlWidgetApp()
    {
        if (udev_notifier_ != nullptr) {
            delete udev_notifier_;
        }
        if (udev_monitor_ != nullptr) {
            udev_monitor_unref(udev_monitor_);
        }
        if (udev_context_ != nullptr) {
            udev_unref(udev_context_);
        }
    }

private:
    void setupTray()
    {
        tray_icon_.setIcon(loadTrayIcon());
        tray_icon_.setToolTip(QObject::tr("HeadsetControl widget"));
        tray_icon_.setVisible(true);
        tray_icon_.show();

        QObject::connect(&tray_icon_, &QSystemTrayIcon::activated, [&](QSystemTrayIcon::ActivationReason reason) {
            if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) {
                refreshState();
                menu_.popup(QCursor::pos());
            }
        });
    }

    void setupMenu()
    {
        status_action_ = menu_.addAction(QObject::tr("Status: Disconnected"));
        status_action_->setEnabled(false);

        battery_action_ = menu_.addAction(QObject::tr("Battery: --"));
        battery_action_->setEnabled(false);

        menu_.addSeparator();
        setupNotificationMenu();
        setupSidetoneMenu();
        setupInactiveMenu();

        menu_.addSeparator();
        auto* refresh_action = menu_.addAction(QObject::tr("Refresh"));
        QObject::connect(refresh_action, &QAction::triggered, [&]() { refreshState(); });

        auto* quit_action = menu_.addAction(QObject::tr("Quit"));
        QObject::connect(quit_action, &QAction::triggered, qApp, &QCoreApplication::quit);

        QObject::connect(&menu_, &QMenu::aboutToShow, [&]() { refreshState(); });
        updateMenuState();
    }

    void setupNotificationMenu()
    {
        auto* notifications_menu = menu_.addMenu(QObject::tr("Battery Notifications"));
        auto* discharge_menu     = notifications_menu->addMenu(QObject::tr("Discharge Level"));
        auto* charge_menu        = notifications_menu->addMenu(QObject::tr("Charge Level"));

        auto* discharge_group = new QActionGroup(discharge_menu);
        discharge_group->setExclusive(true);
        auto* charge_group = new QActionGroup(charge_menu);
        charge_group->setExclusive(true);

        addThresholdAction(discharge_menu, discharge_group, QObject::tr("Disable"), 0, config_.discharge_threshold, [&](int value) {
            config_.discharge_threshold = value;
            saveConfig();
            updateMenuChecks();
        });
        for (int value : DISCHARGE_THRESHOLD_OPTIONS) {
            addThresholdAction(discharge_menu, discharge_group, QObject::tr("%1%").arg(value), value, config_.discharge_threshold, [&](int selected) {
                config_.discharge_threshold = selected;
                saveConfig();
                updateMenuChecks();
            });
        }

        addThresholdAction(charge_menu, charge_group, QObject::tr("Disable"), 0, config_.charge_threshold, [&](int value) {
            config_.charge_threshold = value;
            saveConfig();
            updateMenuChecks();
        });
        for (int value : CHARGE_THRESHOLD_OPTIONS) {
            addThresholdAction(charge_menu, charge_group, QObject::tr("%1%").arg(value), value, config_.charge_threshold, [&](int selected) {
                config_.charge_threshold = selected;
                saveConfig();
                updateMenuChecks();
            });
        }
    }

    void setupSidetoneMenu()
    {
        auto* sidetone_menu  = menu_.addMenu(QObject::tr("Sidetone Level"));
        auto* sidetone_group = new QActionGroup(sidetone_menu);
        sidetone_group->setExclusive(true);

        for (int value : SIDETONE_LEVELS) {
            const int percent = static_cast<int>((value * 100.0) / 128.0 + 0.5);
            auto* action      = sidetone_menu->addAction(QObject::tr("%1 (%2%)").arg(value).arg(percent));
            action->setCheckable(true);
            action->setData(value);
            sidetone_group->addAction(action);
            sidetone_actions_.push_back(action);
            QObject::connect(action, &QAction::triggered, [&, value]() {
                config_.has_sidetone_preference = true;
                config_.sidetone_level          = value;
                pending_sidetone_apply_         = true;
                saveConfig();
                applyPendingSettings();
                updateMenuChecks();
            });
        }
    }

    void setupInactiveMenu()
    {
        auto* inactive_menu  = menu_.addMenu(QObject::tr("Inactive Time / Auto Power-Off"));
        auto* inactive_group = new QActionGroup(inactive_menu);
        inactive_group->setExclusive(true);

        for (int value : INACTIVE_TIME_OPTIONS) {
            QString label = value == 0 ? QObject::tr("0 (Disabled)") : QObject::tr("%1 minute%2").arg(value).arg(value == 1 ? "" : "s");
            auto* action  = inactive_menu->addAction(label);
            action->setCheckable(true);
            action->setData(value);
            inactive_group->addAction(action);
            inactive_actions_.push_back(action);
            QObject::connect(action, &QAction::triggered, [&, value]() {
                config_.has_inactive_preference = true;
                config_.inactive_time_minutes   = value;
                pending_inactive_apply_         = true;
                saveConfig();
                applyPendingSettings();
                updateMenuChecks();
            });
        }
    }

    void addThresholdAction(QMenu* menu, QActionGroup* group, const QString& label, int value, int current_value, const std::function<void(int)>& on_trigger)
    {
        auto* action = menu->addAction(label);
        action->setCheckable(true);
        action->setChecked(current_value == value);
        action->setData(value);
        group->addAction(action);
        QObject::connect(action, &QAction::triggered, [on_trigger, value]() { on_trigger(value); });
    }

    [[nodiscard]] QIcon loadTrayIcon() const
    {
        QIcon icon = QIcon::fromTheme(QStringLiteral("audio-headset"));
        if (!icon.isNull()) {
            return icon;
        }

        const QString app_dir = QCoreApplication::applicationDirPath();
        const QStringList candidates = {
            QDir(app_dir).filePath("../share/pixmaps/headsetcontrol-widget.png"),
            QDir(app_dir).filePath("../assets/headsetcontrol.png"),
            QDir(app_dir).filePath("headsetcontrol.png"),
        };

        for (const auto& path : candidates) {
            if (QFileInfo::exists(path)) {
                return QIcon(path);
            }
        }

        return icon;
    }

    void setupUdevMonitor()
    {
        udev_context_ = udev_new();
        if (udev_context_ == nullptr) {
            return;
        }

        udev_monitor_ = udev_monitor_new_from_netlink(udev_context_, "udev");
        if (udev_monitor_ == nullptr) {
            return;
        }

        udev_monitor_filter_add_match_subsystem_devtype(udev_monitor_, "hidraw", nullptr);
        udev_monitor_enable_receiving(udev_monitor_);

        const int fd = udev_monitor_get_fd(udev_monitor_);
        if (fd < 0) {
            return;
        }

        udev_notifier_ = new QSocketNotifier(fd, QSocketNotifier::Read);
        QObject::connect(udev_notifier_, &QSocketNotifier::activated, [&](int) {
            if (udev_monitor_ == nullptr) {
                return;
            }

            while (auto* device = udev_monitor_receive_device(udev_monitor_)) {
                udev_device_unref(device);
                refreshState();
            }
        });
    }

    void refreshState()
    {
        const DeviceState previous_state = state_;
        const auto previous_level        = state_.connected ? state_.battery_level : std::optional<int> {};

        state_ = queryCurrentState();
        maybeNotify(previous_level, previous_state.power_state, state_);
        updateMenuState();
        applyPendingSettings();
    }

    [[nodiscard]] DeviceState queryCurrentState()
    {
        DeviceState next_state;

        auto result = runHeadsetControl({ QStringLiteral("-o"), QStringLiteral("json") });
        if (!result.ok) {
            updateTooltip(next_state);
            return next_state;
        }

        QJsonParseError parse_error;
        auto json = QJsonDocument::fromJson(result.stdout_text.toUtf8(), &parse_error);
        if (parse_error.error != QJsonParseError::NoError || !json.isObject()) {
            updateTooltip(next_state);
            return next_state;
        }

        const auto root         = json.object();
        const auto device_count = root.value(QStringLiteral("device_count")).toInt();
        if (device_count <= 0) {
            updateTooltip(next_state);
            return next_state;
        }

        const auto devices = root.value(QStringLiteral("devices")).toArray();
        if (devices.isEmpty() || !devices.first().isObject()) {
            updateTooltip(next_state);
            return next_state;
        }

        const auto device_object = devices.first().toObject();
        next_state.connected     = true;
        next_state.device_name   = device_object.value(QStringLiteral("device")).toString(QObject::tr("Headset"));
        next_state.vendor_id     = static_cast<uint16_t>(device_object.value(QStringLiteral("id_vendor")).toInt());
        next_state.product_id    = static_cast<uint16_t>(device_object.value(QStringLiteral("id_product")).toInt());

        const auto battery_object = device_object.value(QStringLiteral("battery")).toObject();
        if (!battery_object.isEmpty()) {
            const auto battery_status = battery_object.value(QStringLiteral("status")).toString();
            if (battery_status == QStringLiteral("BATTERY_CHARGING")) {
                next_state.power_state = PowerState::Charging;
            } else if (battery_status == QStringLiteral("BATTERY_AVAILABLE")) {
                next_state.power_state = PowerState::Discharging;
            } else if (battery_status == QStringLiteral("BATTERY_UNAVAILABLE")) {
                next_state.power_state = PowerState::Unavailable;
            }

            const int level = battery_object.value(QStringLiteral("level")).toInt(-1);
            if (level >= 0) {
                next_state.battery_level = level;
            }
        }

        updateTooltip(next_state);
        return next_state;
    }

    void maybeNotify(const std::optional<int>& previous_level, PowerState previous_state, const DeviceState& current_state)
    {
        const auto decision = evaluateNotificationThresholds(previous_level, previous_state, current_state.battery_level, current_state.power_state, config_.discharge_threshold, config_.charge_threshold);

        if (decision.discharge && current_state.battery_level.has_value()) {
            sendNotification(QObject::tr("Headset battery low"),
                QObject::tr("%1 reached %2% while discharging.").arg(current_state.device_name).arg(*current_state.battery_level));
        }

        if (decision.charge && current_state.battery_level.has_value()) {
            sendNotification(QObject::tr("Headset battery charged"),
                QObject::tr("%1 reached %2% while charging.").arg(current_state.device_name).arg(*current_state.battery_level));
        }
    }

    void sendNotification(const QString& summary, const QString& body)
    {
        QDBusMessage message = QDBusMessage::createMethodCall(
            QStringLiteral("org.freedesktop.Notifications"),
            QStringLiteral("/org/freedesktop/Notifications"),
            QStringLiteral("org.freedesktop.Notifications"),
            QStringLiteral("Notify"));

        message << QStringLiteral("headsetcontrol-widget")
                << static_cast<uint>(0)
                << QStringLiteral("audio-headset")
                << summary
                << body
                << QStringList {}
                << QVariantMap {}
                << 5000;

        QDBusConnection::sessionBus().call(message, QDBus::NoBlock);
    }

    [[nodiscard]] CommandResult runHeadsetControl(const QStringList& arguments) const
    {
        QProcess process;
        process.start(headsetControlExecutable(), arguments);

        CommandResult result;
        if (!process.waitForStarted(COMMAND_TIMEOUT_MS)) {
            result.stderr_text = process.errorString();
            return result;
        }

        if (!process.waitForFinished(COMMAND_TIMEOUT_MS)) {
            process.kill();
            process.waitForFinished(1000);
            result.timed_out = true;
            result.stderr_text = QObject::tr("Timed out waiting for headsetcontrol");
            return result;
        }

        result.exit_code   = process.exitCode();
        result.stdout_text = QString::fromUtf8(process.readAllStandardOutput());
        result.stderr_text = QString::fromUtf8(process.readAllStandardError());
        result.ok          = process.exitStatus() == QProcess::NormalExit && process.exitCode() == 0;
        return result;
    }

    void applyPendingSettings()
    {
        if (!state_.connected) {
            return;
        }

        if (pending_sidetone_apply_ && config_.has_sidetone_preference) {
            auto result = runHeadsetControl(commandWithDeviceFilter({ QStringLiteral("-s"), QString::number(config_.sidetone_level) }));
            if (result.ok) {
                pending_sidetone_apply_ = false;
            }
        }

        if (pending_inactive_apply_ && config_.has_inactive_preference) {
            auto result = runHeadsetControl(commandWithDeviceFilter({ QStringLiteral("-i"), QString::number(config_.inactive_time_minutes) }));
            if (result.ok) {
                pending_inactive_apply_ = false;
            }
        }
    }

    [[nodiscard]] QStringList commandWithDeviceFilter(const QStringList& arguments) const
    {
        QStringList command;
        if (state_.vendor_id != 0 && state_.product_id != 0) {
            command << QStringLiteral("-d")
                    << QStringLiteral("%1:%2")
                           .arg(state_.vendor_id, 4, 16, QChar('0'))
                           .arg(state_.product_id, 4, 16, QChar('0'));
        }
        command << arguments;
        return command;
    }

    [[nodiscard]] QString headsetControlExecutable() const
    {
        const QString app_dir = QCoreApplication::applicationDirPath();
        const QString local   = QDir(app_dir).filePath(QStringLiteral("headsetcontrol"));
        if (QFileInfo::exists(local)) {
            return local;
        }

        const QString from_path = QStandardPaths::findExecutable(QStringLiteral("headsetcontrol"));
        if (!from_path.isEmpty()) {
            return from_path;
        }

        return QStringLiteral("headsetcontrol");
    }

    void updateTooltip(const DeviceState& state)
    {
        QStringList lines;
        lines << (state.connected ? QObject::tr("Status: Connected") : QObject::tr("Status: Disconnected"));
        lines << batterySummary(state);
        lines << QObject::tr("Power: %1").arg(powerStateLabel(state.power_state));
        tray_icon_.setToolTip(lines.join('\n'));
    }

    void updateMenuState()
    {
        status_action_->setText(QObject::tr("Status: %1").arg(state_.connected ? QObject::tr("Connected") : QObject::tr("Disconnected")));
        battery_action_->setText(batterySummary(state_));
        updateMenuChecks();
    }

    void updateMenuChecks()
    {
        for (auto* action : sidetone_actions_) {
            action->setChecked(config_.has_sidetone_preference && action->data().toInt() == config_.sidetone_level);
        }

        for (auto* action : inactive_actions_) {
            action->setChecked(config_.has_inactive_preference && action->data().toInt() == config_.inactive_time_minutes);
        }

        for (auto* action : menu_.actions()) {
            if (auto* submenu = action->menu(); submenu != nullptr) {
                for (auto* child : submenu->actions()) {
                    if (auto* nested = child->menu(); nested != nullptr) {
                        for (auto* nested_action : nested->actions()) {
                            const int value = nested_action->data().toInt();
                            if (nested->title() == QObject::tr("Discharge Level")) {
                                nested_action->setChecked(config_.discharge_threshold == value);
                            } else if (nested->title() == QObject::tr("Charge Level")) {
                                nested_action->setChecked(config_.charge_threshold == value);
                            }
                        }
                    }
                }
            }
        }
    }

    void loadConfig()
    {
        pending_sidetone_apply_ = false;
        pending_inactive_apply_ = false;

        QFile file(config_path_);
        if (!file.exists() || !file.open(QIODevice::ReadOnly)) {
            return;
        }

        const auto json = QJsonDocument::fromJson(file.readAll());
        if (!json.isObject()) {
            return;
        }

        const auto root = json.object();

        const int discharge = root.value(QStringLiteral("discharge_threshold")).toInt(config_.discharge_threshold);
        if (discharge == 0 || contains(DISCHARGE_THRESHOLD_OPTIONS, discharge)) {
            config_.discharge_threshold = discharge;
        }

        const int charge = root.value(QStringLiteral("charge_threshold")).toInt(config_.charge_threshold);
        if (charge == 0 || contains(CHARGE_THRESHOLD_OPTIONS, charge)) {
            config_.charge_threshold = charge;
        }

        const int sidetone = root.value(QStringLiteral("sidetone_level")).toInt(config_.sidetone_level);
        if (root.value(QStringLiteral("has_sidetone_preference")).toBool(false) && contains(SIDETONE_LEVELS, sidetone)) {
            config_.has_sidetone_preference = true;
            config_.sidetone_level          = sidetone;
            pending_sidetone_apply_         = true;
        }

        const int inactive = root.value(QStringLiteral("inactive_time_minutes")).toInt(config_.inactive_time_minutes);
        if (root.value(QStringLiteral("has_inactive_preference")).toBool(false) && contains(INACTIVE_TIME_OPTIONS, inactive)) {
            config_.has_inactive_preference = true;
            config_.inactive_time_minutes   = inactive;
            pending_inactive_apply_         = true;
        }
    }

    void saveConfig() const
    {
        QDir().mkpath(QFileInfo(config_path_).absolutePath());

        QJsonObject root;
        root.insert(QStringLiteral("discharge_threshold"), config_.discharge_threshold);
        root.insert(QStringLiteral("charge_threshold"), config_.charge_threshold);
        root.insert(QStringLiteral("has_sidetone_preference"), config_.has_sidetone_preference);
        root.insert(QStringLiteral("sidetone_level"), config_.sidetone_level);
        root.insert(QStringLiteral("has_inactive_preference"), config_.has_inactive_preference);
        root.insert(QStringLiteral("inactive_time_minutes"), config_.inactive_time_minutes);

        QFile file(config_path_);
        if (file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            file.write(QJsonDocument(root).toJson(QJsonDocument::Indented));
        }
    }

    QSystemTrayIcon tray_icon_;
    QMenu menu_;
    QTimer poll_timer_;
    QString config_path_;
    WidgetConfig config_;
    DeviceState state_;
    QAction* status_action_ = nullptr;
    QAction* battery_action_ = nullptr;
    QList<QAction*> sidetone_actions_;
    QList<QAction*> inactive_actions_;
    bool pending_sidetone_apply_ = false;
    bool pending_inactive_apply_ = false;
    struct udev* udev_context_ = nullptr;
    struct udev_monitor* udev_monitor_ = nullptr;
    QSocketNotifier* udev_notifier_ = nullptr;
};

} // namespace

int main(int argc, char* argv[])
{
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("headsetcontrol-widget"));
    QApplication::setQuitOnLastWindowClosed(false);

    HeadsetControlWidgetApp widget_app;
    return app.exec();
}
