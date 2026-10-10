#!/usr/bin/env python3
"""Exercise the real top terminal on native PTY/ConPTY and save rendered frames.

Requires pyte; Windows additionally requires pywinpty. No camera, mute or
termination mutation is authorized. Run against the extracted release binary.
"""
import argparse
import codecs
import json
import os
from pathlib import Path
import subprocess
import time

import pyte

LANGUAGES = ("en", "fr", "de", "es", "ja", "zh", "ru")

WINDOWS_CLICK = r"""
import ctypes, sys
from ctypes import wintypes as W
kernel = ctypes.WinDLL("kernel32", use_last_error=True)
kernel.AttachConsole.argtypes = [W.DWORD]
kernel.AttachConsole.restype = W.BOOL
kernel.CreateFileW.argtypes = [W.LPCWSTR, W.DWORD, W.DWORD, W.LPVOID, W.DWORD, W.DWORD, W.HANDLE]
kernel.CreateFileW.restype = W.HANDLE
class Coord(ctypes.Structure):
    _fields_ = [("x", W.SHORT), ("y", W.SHORT)]
class SmallRect(ctypes.Structure):
    _fields_ = [(name, W.SHORT) for name in ("left", "top", "right", "bottom")]
class BufferInfo(ctypes.Structure):
    _fields_ = [("size", Coord), ("cursor", Coord), ("attributes", W.WORD),
                ("window", SmallRect), ("maximum", Coord)]
class Mouse(ctypes.Structure):
    _fields_ = [("position", Coord), ("buttons", W.DWORD), ("controls", W.DWORD), ("flags", W.DWORD)]
class Payload(ctypes.Union):
    _fields_ = [("mouse", Mouse), ("padding", ctypes.c_byte * 16)]
class Input(ctypes.Structure):
    _fields_ = [("kind", W.WORD), ("event", Payload)]
kernel.GetConsoleScreenBufferInfo.argtypes = [W.HANDLE, ctypes.POINTER(BufferInfo)]
kernel.GetConsoleScreenBufferInfo.restype = W.BOOL
kernel.WriteConsoleInputW.argtypes = [W.HANDLE, ctypes.POINTER(Input), W.DWORD, ctypes.POINTER(W.DWORD)]
kernel.WriteConsoleInputW.restype = W.BOOL
kernel.CloseHandle.argtypes = [W.HANDLE]
def checked(value):
    if not value:
        raise ctypes.WinError(ctypes.get_last_error())
kernel.FreeConsole()  # This helper alone; never detach the test host or user console.
checked(kernel.AttachConsole(int(sys.argv[1])))
handles = []
try:
    for name, access in [("CONIN$", 0x40000000), ("CONOUT$", 0x80000000)]:
        handle = kernel.CreateFileW(name, access, 3, None, 3, 0, None)
        if handle == W.HANDLE(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        handles.append(handle)
    info = BufferInfo()
    checked(kernel.GetConsoleScreenBufferInfo(handles[1], ctypes.byref(info)))
    records = (Input * 2)()
    for index, buttons in enumerate((1, 0)):
        records[index].kind = 2  # Real MOUSE_EVENT_RECORD; SGR is not Win32 console input.
        records[index].event.mouse = Mouse(
            Coord(int(sys.argv[2]) + info.window.left, int(sys.argv[3]) + info.window.top),
            buttons, 0, 0)
    written = W.DWORD()
    checked(kernel.WriteConsoleInputW(handles[0], records, 2, ctypes.byref(written)))
    assert written.value == 2
finally:
    for handle in handles:
        kernel.CloseHandle(handle)
    kernel.FreeConsole()
"""


class NativeTerminal:
    def __init__(self, binary, language, width, height, *, launch_started=None):
        self.windows = os.name == "nt"
        self.raw = bytearray()
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.screen = pyte.Screen(width, height)
        self.stream = pyte.Stream(self.screen)
        command = [str(binary), "--lang", language, "top"]
        if self.windows:
            from winpty import PtyProcess
            from winpty.enums import Backend
            if launch_started is not None:
                launch_started()
            self.process = PtyProcess.spawn(command, dimensions=(height, width), backend=Backend.ConPTY)
        else:
            import fcntl
            import pty
            import termios
            self.master, slave = pty.openpty()
            self._size(slave, width, height)

            def controlling_terminal():
                os.setsid()
                fcntl.ioctl(0, termios.TIOCSCTTY, 0)

            if launch_started is not None:
                launch_started()
            self.process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave,
                                            preexec_fn=controlling_terminal, close_fds=True)
            os.close(slave)

    @staticmethod
    def _size(fd, width, height):
        import fcntl
        import struct
        import termios
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))

    def alive(self):
        return self.process.isalive() if self.windows else self.process.poll() is None

    def send(self, value):
        if self.windows:
            self.process.write(value)
        else:
            os.write(self.master, value.encode())

    def resize(self, width, height):
        self.screen.resize(lines=height, columns=width)
        if self.windows:
            self.process.setwinsize(height, width)
        else:
            import signal
            self._size(self.master, width, height)
            os.kill(self.process.pid, signal.SIGWINCH)

    def drain(self, seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if self.windows:
                import select
                if not select.select([self.process.fileobj], [], [], .05)[0]:
                    continue
                try:
                    text = self.process.read(65536)
                except EOFError:
                    break
                if not text:
                    time.sleep(.02)
                    continue
                self.raw.extend(text.encode("utf-8"))
            else:
                import errno
                import select
                if not select.select([self.master], [], [], .05)[0]:
                    continue
                try:
                    chunk = os.read(self.master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        break
                    raise
                if not chunk:
                    break
                self.raw.extend(chunk)
                text = self.decoder.decode(chunk)
            self.stream.feed(text)

    @property
    def display(self):
        # pyte's convenience renderer indexes empty wide-character continuation
        # cells and can crash after ConPTY resize. The authoritative cell grid
        # already represents those continuations as empty strings.
        return ["".join(self.screen.buffer[y][x].data for x in range(self.screen.columns))
                for y in range(self.screen.lines)]

    def locate(self, key):
        marker = "[" + key + "]"
        # display strings omit wide-character continuation cells; native mouse
        # coordinates must use physical cells, not Python string offsets.
        for y in range(self.screen.lines):
            for x in range(self.screen.columns - 2):
                if "".join(self.screen.buffer[y][x + offset].data for offset in range(3)) == marker:
                    return x, y
        raise AssertionError(f"No visible {marker} button: {self.display}")

    def click(self, x, y):
        if self.windows:
            # Attach a disposable helper only to this owned ConPTY child.
            # Writing SGR bytes can be interpreted as keyboard letters by Win32
            # console readers, and must never accidentally invoke mute controls.
            import sys
            result = subprocess.run([sys.executable, "-B", "-c", WINDOWS_CLICK,
                                     str(self.process.pid), str(x), str(y)],
                                    creationflags=subprocess.CREATE_NO_WINDOW, capture_output=True)
            if result.returncode:
                raise RuntimeError("Native ConPTY mouse input failed: " +
                                   result.stderr.decode("utf-8", "replace"))
        else:
            self.send(f"\x1b[<0;{x + 1};{y + 1}M\x1b[<0;{x + 1};{y + 1}m")

    def frame(self, path):
        path.with_suffix(".txt").write_text("\n".join(self.display) + "\n", encoding="utf-8")
        cells = [[dict(text=cell.data, fg=cell.fg, bg=cell.bg, bold=cell.bold)
                  for x in range(self.screen.columns)
                  for cell in [self.screen.buffer[y][x]]]
                 for y in range(self.screen.lines)]
        path.with_suffix(".json").write_text(json.dumps(cells, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    def close(self):
        if self.alive():
            if self.windows:
                self.process.terminate(force=True)
            else:
                import signal
                os.killpg(self.process.pid, signal.SIGTERM)
                self.process.wait(timeout=20)
        if self.windows:
            self.process.close(force=True)
        else:
            os.close(self.master)


def exercise(binary, proof, language):
    terminal = NativeTerminal(binary, language, 120, 40)
    try:
        terminal.drain(6)
        assert terminal.alive(), "top exited before input"
        assert len({terminal.locate(key)[1] for key in "bamkrq"}) == 1, \
            "The six visible actions must share one horizontal row"
        terminal.frame(proof / (language + "-120x40"))
        if os.name != "nt" and os.uname().sysname == "Linux":
            for key in "ba":
                x, y = terminal.locate(key)
                before = terminal.display[-4:-1]
                terminal.click(x, y)
                terminal.drain(.2)
                assert terminal.display[-4:-1] == before, "Disabled mouse button changed feedback"
                terminal.send(key.upper())
                terminal.drain(.3)
                assert terminal.alive(), "Unavailable Linux camera control exited top"
        rx, ry = terminal.locate("r")
        terminal.click(rx, ry)
        terminal.drain(.3)
        terminal.frame(proof / (language + "-mouse-refresh"))
        terminal.send("R")
        terminal.drain(.3)
        assert terminal.alive(), "Read-only refresh exited top"
        for width, height in ((40, 24), (20, 12), (150, 40), (120, 40)):
            terminal.resize(width, height)
            terminal.drain(.8)
            terminal.locate("q")
            if width >= 120:
                assert len({terminal.locate(key)[1] for key in "bamkrq"}) == 1, \
                    "Resizing stacked the action bar"
            terminal.locate("r")
            assert terminal.alive(), "Resize exited top"
            terminal.frame(proof / (language + f"-{width}x{height}-resized"))
            if width == 150:
                qx, qy = terminal.locate("q")
                row = terminal.screen.buffer[qy]
                last_painted = max(x for x in range(qx, terminal.screen.columns)
                                   if row[x].data not in ("", " "))
                terminal.click(last_painted + 1, qy)
                terminal.drain(.3)
                assert terminal.alive(), "Clicking outside the visible Quit button executed Quit"
            terminal.send("\x1b[A\x1b[B")  # Preserve process-selection navigation.
            terminal.drain(.2)
        # Use native mouse dispatch for one session, case-insensitive keyboard
        # dispatch for the remaining translations. Never invoke device controls.
        if language == "en":
            terminal.click(*terminal.locate("q"))
        else:
            terminal.send("Q")
        terminal.drain(2)
        deadline = time.monotonic() + 15
        while terminal.alive() and time.monotonic() < deadline:
            terminal.drain(.1)
        assert not terminal.alive(), "Visible Quit did not exit top"
        code = terminal.process.exitstatus if terminal.windows else terminal.process.wait(timeout=2)
        assert code == 0, ("top exit", code)
    finally:
        terminal.close()
        (proof / (language + ".ansi")).write_bytes(terminal.raw)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--proof", type=Path, required=True)
    args = parser.parse_args()
    args.proof.mkdir(parents=True, exist_ok=True)
    for language in LANGUAGES:
        exercise(args.binary.resolve(), args.proof, language)
    (args.proof / "scope.txt").write_text(
        "Actual release top on native PTY/ConPTY; seven languages, terminal resize, "
        "safe refresh by mouse/key, inert Quit gap, visible mouse/uppercase-key Quit. "
        "Rendered terminal frames retain cell colors for visual review. Linux camera "
        "controls are invoked only as disabled actions. No camera profile, microphone "
        "mute or process termination is authorized; no hardware activity claim.\n", encoding="utf-8")


if __name__ == "__main__":
    main()
