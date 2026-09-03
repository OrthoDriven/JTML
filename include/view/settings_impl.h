#ifndef SETTINGS_IMPL_H_
#define SETTINGS_IMPL_H_

enum class SettingsImpl {
    CppWidgets,
    RustQml,
};

inline SettingsImpl& settings_impl_storage() {
    static SettingsImpl impl = SettingsImpl::RustQml;
    return impl;
}

inline SettingsImpl settings_impl() {
    return settings_impl_storage();
}

inline void set_settings_impl(SettingsImpl impl) {
    settings_impl_storage() = impl;
}

#endif  // SETTINGS_IMPL_H_
