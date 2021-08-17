fn main() {
    windows::build! {
        Windows::Win32::Foundation::CloseHandle,
        Windows::Win32::System::Console::*,
        Windows::Win32::System::Pipes::*,
        Windows::Win32::System::Threading::*,
        Windows::Win32::System::WindowsProgramming::INFINITE,
        Windows::Win32::Storage::FileSystem::*,
    };
}
