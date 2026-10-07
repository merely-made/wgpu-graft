"""Passive, bounded own-bundle observation. No activation, input or pixels."""

import argparse
import ctypes as C
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def optional_bool(value):
    return value if type(value) is bool else None


def optional_number(value):
    return value if type(value) in (int, float) else None


def owned_paths(path, app_bundle, bundle):
    return path == str(bundle / "Contents/MacOS/demo-mac") and app_bundle == str(bundle)


def session_fields(values):
    # Key spellings are optional compatibility lookups, never a lock inference.
    return {"session_uid": optional_number(values.get("kCGSSessionUserIDKey")),
            "on_console": optional_bool(values.get("kCGSSessionOnConsoleKey")),
            "login_done": optional_bool(values.get("kCGSessionLoginDoneKey")),
            "locked": optional_bool(values.get("CGSSessionScreenIsLocked"))}


class MacFacts:
    def __init__(self):
        self.cf = C.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
        self.cg = C.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
        C.CDLL("/System/Library/Frameworks/AppKit.framework/AppKit")
        self.objc = C.CDLL("/usr/lib/libobjc.A.dylib")
        self.objc.objc_getClass.argtypes = [C.c_char_p]
        self.objc.objc_getClass.restype = C.c_void_p
        self.objc.sel_registerName.argtypes = [C.c_char_p]
        self.objc.sel_registerName.restype = C.c_void_p
        self.message = C.cast(self.objc.objc_msgSend, C.c_void_p).value
        self.bind(self.cg, "CGSessionCopyCurrentDictionary", C.c_void_p, [])
        self.bind(self.cg, "CGWindowListCopyWindowInfo", C.c_void_p, [C.c_uint32, C.c_uint32])
        for name in ("CFGetTypeID", "CFBooleanGetTypeID", "CFNumberGetTypeID"):
            self.bind(self.cf, name, C.c_ulong, [C.c_void_p] if name == "CFGetTypeID" else [])
        self.bind(self.cf, "CFBooleanGetValue", C.c_bool, [C.c_void_p])
        self.bind(self.cf, "CFNumberGetValue", C.c_bool, [C.c_void_p, C.c_int, C.c_void_p])
        self.bind(self.cf, "CFDictionaryGetValue", C.c_void_p, [C.c_void_p, C.c_void_p])
        self.bind(self.cf, "CFArrayGetCount", C.c_long, [C.c_void_p])
        self.bind(self.cf, "CFArrayGetValueAtIndex", C.c_void_p, [C.c_void_p, C.c_long])
        self.bind(self.cf, "CFStringCreateWithCString", C.c_void_p, [C.c_void_p, C.c_char_p, C.c_uint32])
        self.bind(self.cf, "CFRelease", None, [C.c_void_p])
        self.bind(self.cf, "CFRunLoopRunInMode", C.c_int32, [C.c_void_p, C.c_double, C.c_bool])
        self.mode = C.c_void_p.in_dll(self.cf, "kCFRunLoopDefaultMode").value

    @staticmethod
    def bind(library, name, result, arguments):
        function = getattr(library, name)
        function.restype, function.argtypes = result, arguments

    def msg(self, receiver, selector, result=C.c_void_p, arguments=(), values=()):
        if not receiver:
            return None
        function = C.CFUNCTYPE(result, C.c_void_p, C.c_void_p, *arguments)(self.message)
        return function(receiver, self.objc.sel_registerName(selector.encode()), *values)

    def cls(self, name):
        return self.objc.objc_getClass(name.encode())

    def string(self, value):
        return self.msg(self.cls("NSString"), "stringWithUTF8String:", arguments=(C.c_char_p,), values=(value.encode(),))

    def text(self, value):
        raw = self.msg(value, "UTF8String", C.c_char_p)
        return raw.decode("utf-8") if raw is not None else None

    def lookup(self, dictionary, key):
        if not dictionary:
            return None
        string = self.cf.CFStringCreateWithCString(None, key.encode(), 0x08000100)
        try:
            return self.cf.CFDictionaryGetValue(dictionary, string)
        finally:
            self.cf.CFRelease(string)

    def scalar(self, value):
        if not value:
            return None
        kind = self.cf.CFGetTypeID(value)
        if kind == self.cf.CFBooleanGetTypeID():
            return bool(self.cf.CFBooleanGetValue(value))
        if kind == self.cf.CFNumberGetTypeID():
            number = C.c_double()
            return number.value if self.cf.CFNumberGetValue(value, 6, C.byref(number)) else None
        return None

    def facts(self, bundle, bundle_id):
        # Public AppKit time-varying properties refresh on a main-run-loop turn.
        self.cf.CFRunLoopRunInMode(self.mode, 0.01, False)
        pool = self.msg(self.cls("NSAutoreleasePool"), "new")
        session = windows = None
        try:
            session = self.cg.CGSessionCopyCurrentDictionary()
            fields = session_fields({key: self.scalar(self.lookup(session, key)) for key in (
                "kCGSSessionUserIDKey", "kCGSSessionOnConsoleKey", "kCGSessionLoginDoneKey", "CGSSessionScreenIsLocked")})
            apps = self.msg(self.cls("NSRunningApplication"), "runningApplicationsWithBundleIdentifier:", values=(self.string(bundle_id),), arguments=(C.c_void_p,))
            own = [] if apps else None
            for index in range(self.msg(apps, "count", C.c_ulong) or 0):
                app = self.msg(apps, "objectAtIndex:", arguments=(C.c_ulong,), values=(index,))
                path = self.text(self.msg(self.msg(app, "executableURL"), "path"))
                app_bundle = self.text(self.msg(self.msg(app, "bundleURL"), "path"))
                if not owned_paths(path, app_bundle, bundle):
                    continue
                own.append({"pid": self.msg(app, "processIdentifier", C.c_int),
                            "path": "$SCRY_MAC_APP/Contents/MacOS/demo-mac", "path_verified_exact": True,
                            "activation_policy": self.msg(app, "activationPolicy", C.c_long),
                            "is_active": self.msg(app, "isActive", C.c_bool),
                            "is_hidden": self.msg(app, "isHidden", C.c_bool)})
            workspace = self.msg(self.cls("NSWorkspace"), "sharedWorkspace")
            front = self.msg(workspace, "frontmostApplication")
            front_pid = self.msg(front, "processIdentifier", C.c_int)
            front_id = self.text(self.msg(front, "bundleIdentifier"))
            # The all-window dictionary is projected only after exact own-PID filtering.
            windows = self.cg.CGWindowListCopyWindowInfo(16, 0)
            own_windows = [] if windows and own is not None else None
            own_pids = {app["pid"] for app in own or []}
            for index in range(self.cf.CFArrayGetCount(windows) if windows else 0):
                window = self.cf.CFArrayGetValueAtIndex(windows, index)
                pid = self.scalar(self.lookup(window, "kCGWindowOwnerPID"))
                if pid not in own_pids:
                    continue
                bounds = self.lookup(window, "kCGWindowBounds")
                own_windows.append({"pid": int(pid),
                                    "on_screen": optional_bool(self.scalar(self.lookup(window, "kCGWindowIsOnscreen"))),
                                    "layer": self.scalar(self.lookup(window, "kCGWindowLayer")),
                                    "alpha": self.scalar(self.lookup(window, "kCGWindowAlpha")),
                                    "extent": {key: self.scalar(self.lookup(bounds, key)) for key in ("X", "Y", "Width", "Height")}})
            return {**fields, "session_dictionary_available": bool(session), "own_apps": own,
                    "window_list_available": bool(windows), "own_windows": own_windows,
                    "frontmost_is_own": front_pid in own_pids if front else None,
                    "frontmost_is_loginwindow": front_id == "com.apple.loginwindow" if front_id else None}
        finally:
            for value in (session, windows):
                if value:
                    self.cf.CFRelease(value)
            self.msg(pool, "drain", None)


def observe(bundle, native):
    try:
        console_uid = os.stat("/dev/console").st_uid
    except OSError:
        console_uid = None
    try:
        status = subprocess.run(["/usr/bin/pgrep", "-x", "WindowServer"], stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL, timeout=1).returncode
        window_server = True if status == 0 else False if status == 1 else None
    except (OSError, subprocess.TimeoutExpired):
        window_server = None
    facts = {"runner_uid": os.getuid(), "console_uid": console_uid,
             "runner_matches_console_uid": os.getuid() == console_uid if console_uid is not None else None,
             "window_server_present": window_server}
    if native is not None:
        try:
            return {**facts, **native.facts(bundle, "org.merely.scry.hardware-demo")}
        except Exception as error:
            facts["native_sample_error_type"] = type(error).__name__
    return {**facts, **session_fields({}), "session_dictionary_available": None, "own_apps": None,
            "window_list_available": None, "own_windows": None, "frontmost_is_own": None, "frontmost_is_loginwindow": None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proof", type=Path, required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    args = parser.parse_args()
    proof, bundle = args.proof.resolve(), args.bundle.resolve()
    started = time.monotonic()
    native, initialization_error = None, None
    try:
        if sys.platform != "darwin":
            raise OSError("macOS required")
        native = MacFacts()
    except Exception as error:
        initialization_error = type(error).__name__
    state = {"pid": os.getpid(), "interval_seconds": 3, "max_seconds": 1800, "max_samples": 600,
             "helper_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
             "bundle_path_sha256": hashlib.sha256(str(bundle).encode()).hexdigest(),
             "native_api_initialized": native is not None, "initialization_error_type": initialization_error,
             "session_key_policy": "explicit optional compatibility spellings; unavailable/wrong-type means null, never an unlock inference"}
    (proof / "session-observer-start.json").write_text(json.dumps(state, indent=2) + "\n")
    count = 0
    with (proof / "session-observations.jsonl").open("x", encoding="utf-8") as output:
        while count < 600 and time.monotonic() - started < 1800 and not (proof / "session-observer.stop").exists():
            phase = "before-baseline"
            for name in ("ordinary", "activity", "foreground-ordinary", "foreground-activity"):
                if (proof / (name + ".log")).exists():
                    phase = name
            sample = {"utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                      "elapsed_seconds": time.monotonic() - started, "phase": phase,
                      **observe(bundle, native)}
            output.write(json.dumps(sample, sort_keys=True, allow_nan=False) + "\n")
            output.flush()
            count += 1
            until = time.monotonic() + 3
            while time.monotonic() < until and not (proof / "session-observer.stop").exists():
                time.sleep(0.1)
    (proof / "session-observer-finish.json").write_text(json.dumps({"pid": os.getpid(), "sample_count": count,
        "elapsed_seconds": time.monotonic() - started, "stop_requested": (proof / "session-observer.stop").exists(),
        "native_api_initialized": native is not None, "bounded_observation_only": True}, indent=2) + "\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Do not echo exceptions that may contain unredacted paths or values.
        sys.stderr.write(type(error).__name__ + "\n")
        raise SystemExit(1)
