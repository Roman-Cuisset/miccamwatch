"""Hosted-only diagnostic; compare original and genuine ordinary account contexts.
No ACL/token/policy changes. Fresh diagnostic objects close immediately.
"""
import ctypes as C
from ctypes import wintypes as W
import json
import os
import shutil
import uuid

if os.name != 'nt' or os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted':
    raise SystemExit('Disposable GitHub-hosted Windows runner required')

kernel = C.WinDLL('kernel32', use_last_error=True)
advapi = C.WinDLL('advapi32', use_last_error=True)
ntdll = C.WinDLL('ntdll', use_last_error=True)

def bind(dll, name, result, args):
    fn = getattr(dll, name)
    fn.restype, fn.argtypes = result, args
    return fn

close = bind(kernel, 'CloseHandle', W.BOOL, [W.HANDLE])
free = bind(kernel, 'LocalFree', C.c_void_p, [C.c_void_p])
get_pid = bind(kernel, 'GetCurrentProcessId', W.DWORD, [])
get_session = bind(kernel, 'ProcessIdToSessionId', W.BOOL, [W.DWORD, C.POINTER(W.DWORD)])
semaphore = bind(kernel, 'CreateSemaphoreA', W.HANDLE, [C.c_void_p, W.LONG, W.LONG, C.c_char_p])
mutex = bind(kernel, 'CreateMutexA', W.HANDLE, [C.c_void_p, W.BOOL, C.c_char_p])
get_security = bind(advapi, 'GetSecurityInfo', W.DWORD, [W.HANDLE, C.c_int, W.DWORD, C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p, C.POINTER(C.c_void_p)])
sddl = bind(advapi, 'ConvertSecurityDescriptorToStringSecurityDescriptorW', W.BOOL, [C.c_void_p, W.DWORD, W.DWORD, C.POINTER(C.c_void_p), C.c_void_p])

class UnicodeString(C.Structure):
    _fields_ = [('Length', W.USHORT), ('MaximumLength', W.USHORT), ('Buffer', C.c_void_p)]

class ObjectAttributes(C.Structure):
    _fields_ = [('Length', W.ULONG), ('RootDirectory', W.HANDLE), ('ObjectName', C.POINTER(UnicodeString)), ('Attributes', W.ULONG), ('SecurityDescriptor', C.c_void_p), ('SecurityQualityOfService', C.c_void_p)]

open_directory = bind(ntdll, 'NtOpenDirectoryObject', W.LONG, [C.POINTER(W.HANDLE), W.DWORD, C.POINTER(ObjectAttributes)])
query_object = bind(ntdll, 'NtQueryObject', W.LONG, [W.HANDLE, C.c_int, C.c_void_p, W.ULONG, C.POINTER(W.ULONG)])

def object_name(handle):
    buffer = C.create_string_buffer(8192)
    needed = W.ULONG()
    status = query_object(handle, 1, buffer, len(buffer), C.byref(needed))
    if status < 0:
        return {'ntstatus': hex(status & 0xffffffff)}
    value = C.cast(buffer, C.POINTER(UnicodeString)).contents
    return C.wstring_at(value.Buffer, value.Length // 2) if value.Buffer else ''

def security(handle, information=4):
    descriptor = C.c_void_p()
    result = get_security(handle, 6, information, None, None, None, None, C.byref(descriptor))
    if result:
        return {'GetSecurityInfo_error': result}
    text = C.c_void_p()
    try:
        if not sddl(descriptor, 1, information, C.byref(text), None):
            return {'ConvertSecurityDescriptor_error': C.get_last_error()}
        return C.wstring_at(text)
    finally:
        if text:
            free(text)
        if descriptor:
            free(descriptor)

session = W.DWORD()
if not get_session(get_pid(), C.byref(session)):
    raise C.WinError(C.get_last_error())
report = {'pid': get_pid(), 'session_id': session.value,
          'executables': {name: shutil.which(name) for name in ('cargo', 'rustc', 'link', 'cl')},
          'environment': {name: os.environ.get(name) for name in ('USERPROFILE', 'TEMP', 'TMP', 'CARGO_HOME', 'RUSTUP_HOME', 'RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_MAKEFLAGS', 'MAKEFLAGS', 'MFLAGS')},
          'objects': [], 'directories': []}
for kind in ('semaphore', 'mutex'):
    for prefix in (None, '', 'Local\\', 'Global\\'):
        name = None if prefix is None else (prefix + 'mcw_context_probe_' + uuid.uuid4().hex).encode('ascii')
        C.set_last_error(0)
        handle = semaphore(None, 1, 1, name) if kind == 'semaphore' else mutex(None, False, name)
        error = C.get_last_error()
        item = {'kind': kind, 'requested_name': None if name is None else name.decode('ascii'), 'success': bool(handle), 'win32_error': error}
        if handle:
            try:
                item.update(actual_object_name=object_name(handle), dacl=security(handle), mandatory_label=security(handle, 16))
            finally:
                close(handle)
        report['objects'].append(item)
for path in ('\\BaseNamedObjects', f'\\Sessions\\{session.value}\\BaseNamedObjects'):
    for access in (4, 0x20000):
        buffer = C.create_unicode_buffer(path)
        name = UnicodeString(len(path.encode('utf-16-le')), C.sizeof(buffer), C.cast(buffer, C.c_void_p))
        attributes = ObjectAttributes(C.sizeof(ObjectAttributes), None, C.pointer(name), 0x40, None, None)
        handle = W.HANDLE()
        status = open_directory(C.byref(handle), access, C.byref(attributes))
        item = {'path': path, 'requested_access': hex(access), 'ntstatus': hex(status & 0xffffffff)}
        if status >= 0:
            try:
                if access == 0x20000:
                    item.update(dacl=security(handle), mandatory_label=security(handle, 16))
            finally:
                close(handle)
        report['directories'].append(item)
print(json.dumps(report, indent=2))
