#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <cstdio>
#include <cstdint>
#include <fcntl.h>
#include <io.h>

#include "flutter_window.h"
#include "utils.h"

namespace {

bool IsRedirected(HANDLE handle) {
  return handle != nullptr && handle != INVALID_HANDLE_VALUE &&
         GetFileType(handle) != FILE_TYPE_CHAR;
}

// GUI executables may have a valid inherited Win32 pipe but no CRT stdout.
// Rebind the CRT without attaching a console or replacing the parent's pipe.
bool BindRedirectedStream(DWORD kind, HANDLE inherited, FILE* stream,
                          bool reading = false) {
  const int existing_descriptor = _fileno(stream);
  if (existing_descriptor >= 0) {
    const intptr_t existing_handle = _get_osfhandle(existing_descriptor);
    if (existing_handle != -1 && existing_handle != -2) {
      // The CRT already inherited this stream. In particular, do not close a
      // shared stdout/stderr pipe while rebinding the other stream.
      return true;
    }
  }
  HANDLE duplicate = nullptr;
  if (!DuplicateHandle(GetCurrentProcess(), inherited, GetCurrentProcess(),
                       &duplicate, 0, FALSE, DUPLICATE_SAME_ACCESS)) {
    return false;
  }
  const int descriptor =
      _open_osfhandle(reinterpret_cast<intptr_t>(duplicate),
                     _O_BINARY | (reading ? _O_RDONLY : _O_WRONLY));
  if (descriptor == -1) {
    CloseHandle(duplicate);
    return false;
  }
  FILE* reopened = nullptr;
  const bool success = freopen_s(&reopened, "NUL", reading ? "r" : "w", stream) == 0 &&
                       _dup2(descriptor, _fileno(stream)) == 0;
  _close(descriptor);
  if (!success) {
    return false;
  }
  _setmode(_fileno(stream), _O_BINARY);
  setvbuf(stream, nullptr, _IONBF, 0);
  return SetStdHandle(
             kind, reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stream)))) !=
         FALSE;
}

}  // namespace

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  const HANDLE input = GetStdHandle(STD_INPUT_HANDLE);
  const HANDLE output = GetStdHandle(STD_OUTPUT_HANDLE);
  const HANDLE error = GetStdHandle(STD_ERROR_HANDLE);
  // This GUI is only a child presentation process. Never attach a console.
  if (!IsRedirected(input) || !IsRedirected(output) || !IsRedirected(error) ||
      !BindRedirectedStream(STD_INPUT_HANDLE, input, stdin, true) ||
      !BindRedirectedStream(STD_OUTPUT_HANDLE, output, stdout) ||
      !BindRedirectedStream(STD_ERROR_HANDLE, error, stderr)) {
    return EXIT_FAILURE;
  }
  StartRunnerTiming();
  EmitRunnerTiming("runner_started");

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();
  const bool dark = [&]() {
    for (size_t i = 0; i < command_line_arguments.size(); ++i) {
      if (command_line_arguments[i] == "--theme=dark") return true;
      if (command_line_arguments[i] == "--theme" &&
          i + 1 < command_line_arguments.size()) {
        return command_line_arguments[i + 1] == "dark";
      }
    }
    return false;
  }();

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project, dark);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(1040, 700);
  if (!window.Create(L"MicCamWatch", origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  return EXIT_SUCCESS;
}
