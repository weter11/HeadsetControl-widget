#pragma once

#include <array>
#include <optional>

namespace headsetcontrol::widget {

inline constexpr std::array<int, 10> SIDETONE_LEVELS = { 0, 14, 28, 42, 57, 71, 85, 99, 113, 128 };
inline constexpr std::array<int, 12> INACTIVE_TIME_OPTIONS = { 0, 1, 2, 3, 5, 10, 15, 20, 30, 45, 60, 90 };
inline constexpr std::array<int, 9> DISCHARGE_THRESHOLD_OPTIONS = { 10, 20, 30, 40, 50, 60, 70, 80, 90 };
inline constexpr std::array<int, 10> CHARGE_THRESHOLD_OPTIONS = { 10, 20, 30, 40, 50, 60, 70, 80, 90, 100 };

enum class PowerState {
    Unknown,
    Charging,
    Discharging,
    Unavailable,
};

template <size_t N>
[[nodiscard]] constexpr bool contains(const std::array<int, N>& values, int needle)
{
    for (int value : values) {
        if (value == needle) {
            return true;
        }
    }
    return false;
}

struct NotificationDecision {
    bool discharge = false;
    bool charge    = false;
};

[[nodiscard]] inline NotificationDecision evaluateNotificationThresholds(
    const std::optional<int>& previous_level,
    PowerState /*previous_state*/,
    const std::optional<int>& current_level,
    PowerState current_state,
    int discharge_threshold,
    int charge_threshold)
{
    NotificationDecision decision;

    if (!previous_level.has_value() || !current_level.has_value()) {
        return decision;
    }

    if (discharge_threshold > 0 && current_state == PowerState::Discharging && *previous_level > discharge_threshold && *current_level <= discharge_threshold) {
        decision.discharge = true;
    }

    if (charge_threshold > 0 && current_state == PowerState::Charging && *previous_level < charge_threshold && *current_level >= charge_threshold) {
        decision.charge = true;
    }

    return decision;
}

} // namespace headsetcontrol::widget
