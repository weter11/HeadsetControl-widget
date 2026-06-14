#include "widget_logic.hpp"

#include <iostream>
#include <sstream>
#include <stdexcept>

namespace headsetcontrol::testing {

class TestFailure : public std::runtime_error {
public:
    explicit TestFailure(const std::string& message)
        : std::runtime_error(message)
    {
    }
};

#define ASSERT_TRUE(cond, msg)                                              \
    do {                                                                    \
        if (!(cond)) {                                                      \
            throw TestFailure(std::string("ASSERT_TRUE failed: ") + (msg)); \
        }                                                                   \
    } while (0)

#define ASSERT_FALSE(cond, msg)                                              \
    do {                                                                     \
        if ((cond)) {                                                        \
            throw TestFailure(std::string("ASSERT_FALSE failed: ") + (msg)); \
        }                                                                    \
    } while (0)

#define ASSERT_EQ(expected, actual, msg)                 \
    do {                                                 \
        if ((expected) != (actual)) {                    \
            std::ostringstream oss;                      \
            oss << "ASSERT_EQ failed: " << (msg) << "\n" \
                << "  Expected: " << (expected) << "\n"  \
                << "  Actual:   " << (actual);           \
            throw TestFailure(oss.str());                \
        }                                                \
    } while (0)

void testThresholdCrossingLogic()
{
    std::cout << "  Testing widget notification thresholds..." << std::endl;

    auto low = widget::evaluateNotificationThresholds(35, widget::PowerState::Discharging, 20, widget::PowerState::Discharging, 20, 80);
    ASSERT_TRUE(low.discharge, "discharge threshold should trigger when crossed downward");
    ASSERT_FALSE(low.charge, "charge threshold should not trigger on discharge");

    auto charge = widget::evaluateNotificationThresholds(70, widget::PowerState::Charging, 80, widget::PowerState::Charging, 20, 80);
    ASSERT_FALSE(charge.discharge, "discharge threshold should not trigger while charging");
    ASSERT_TRUE(charge.charge, "charge threshold should trigger when crossed upward");

    auto unchanged = widget::evaluateNotificationThresholds(19, widget::PowerState::Discharging, 18, widget::PowerState::Discharging, 20, 80);
    ASSERT_FALSE(unchanged.discharge, "already-below threshold should not retrigger");

    auto disabled = widget::evaluateNotificationThresholds(85, widget::PowerState::Charging, 100, widget::PowerState::Charging, 0, 0);
    ASSERT_FALSE(disabled.discharge, "disabled discharge threshold should not trigger");
    ASSERT_FALSE(disabled.charge, "disabled charge threshold should not trigger");

    std::cout << "    OK widget notification thresholds" << std::endl;
}

void testWidgetOptionSets()
{
    std::cout << "  Testing widget option sets..." << std::endl;

    ASSERT_EQ(10, static_cast<int>(widget::SIDETONE_LEVELS.size()), "sidetone should expose exactly ten points");
    ASSERT_EQ(0, widget::SIDETONE_LEVELS.front(), "sidetone should start at 0");
    ASSERT_EQ(128, widget::SIDETONE_LEVELS.back(), "sidetone should end at 128");
    ASSERT_TRUE(widget::contains(widget::INACTIVE_TIME_OPTIONS, 45), "inactive time should include 45 minutes");
    ASSERT_TRUE(widget::contains(widget::CHARGE_THRESHOLD_OPTIONS, 100), "charge thresholds should include 100%");
    ASSERT_FALSE(widget::contains(widget::DISCHARGE_THRESHOLD_OPTIONS, 100), "discharge thresholds should stop at 90%");

    std::cout << "    OK widget option sets" << std::endl;
}

void runAllWidgetLogicTests()
{
    std::cout << "\n=== Widget Logic Tests ===" << std::endl;
    testThresholdCrossingLogic();
    testWidgetOptionSets();
}

} // namespace headsetcontrol::testing
