#include "utils.h"

#include <stdio.h>
#include <windows.h>


namespace {
ULONGLONG runner_started = 0;
}

void StartRunnerTiming() {
  runner_started = GetTickCount64();
}

void EmitRunnerTiming(const char* stage) {
  char event[256];
  const int length = snprintf(
      event, sizeof(event),
      "{\"type\":\"timing\",\"protocol_version\":5,\"stage\":\"%s\","
      "\"elapsed_ms\":%llu}\n",
      stage, GetTickCount64() - runner_started);
  if (length <= 0 || static_cast<size_t>(length) >= sizeof(event)) return;
  fflush(stdout);
  DWORD written = 0;
  // One pipe write keeps the bounded event intact beside engine logging.
  WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), event,
            static_cast<DWORD>(length), &written, nullptr);
}

std::vector<std::string> GetCommandLineArguments() {
  // Convert the UTF-16 command line arguments to UTF-8 for the Engine to use.
  int argc;
  wchar_t** argv = ::CommandLineToArgvW(::GetCommandLineW(), &argc);
  if (argv == nullptr) {
    return std::vector<std::string>();
  }

  std::vector<std::string> command_line_arguments;

  // Skip the first argument as it's the binary name.
  for (int i = 1; i < argc; i++) {
    command_line_arguments.push_back(Utf8FromUtf16(argv[i]));
  }

  ::LocalFree(argv);

  return command_line_arguments;
}

std::string Utf8FromUtf16(const wchar_t* utf16_string) {
  if (utf16_string == nullptr) {
    return std::string();
  }
  // First, find the length of the string with a safe upper bound (CWE-126).
  // UNICODE_STRING_MAX_CHARS (32767) is the maximum length of a UNICODE_STRING.
  int input_length = static_cast<int>(wcsnlen(utf16_string, UNICODE_STRING_MAX_CHARS));
  // Now use that bounded length to determine the required buffer size.
  // When an explicit length is passed, WideCharToMultiByte does not include
  // the null terminator in its returned size.
  int target_length = ::WideCharToMultiByte(
      CP_UTF8, WC_ERR_INVALID_CHARS, utf16_string,
      input_length, nullptr, 0, nullptr, nullptr);
  std::string utf8_string;
  if (target_length == 0 || static_cast<size_t>(target_length) > utf8_string.max_size()) {
    return utf8_string;
  }
  utf8_string.resize(target_length);
  int converted_length = ::WideCharToMultiByte(
      CP_UTF8, WC_ERR_INVALID_CHARS, utf16_string,
      input_length, utf8_string.data(), target_length, nullptr, nullptr);
  if (converted_length == 0) {
    return std::string();
  }
  return utf8_string;
}
