fn main() {
    windows::build! {
        Windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, MAX_PATH},
        Windows::Win32::System::Console::*,
        Windows::Win32::System::Pipes::*,
        Windows::Win32::System::Threading::*,
        Windows::Win32::System::SystemServices::*,
        Windows::Win32::System::WindowsProgramming::INFINITE,
        Windows::Win32::Storage::FileSystem::*,
        Windows::Win32::Security::SECURITY_ATTRIBUTES,
    };
}
