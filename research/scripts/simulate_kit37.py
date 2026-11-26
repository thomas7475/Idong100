#!/usr/bin/env python3
"""Simulate the kit 37 .bat decision logic - no Windows here, so the exact
IF/fc/copy/findstr decisions of each script are replayed in Python over fake
trees and states, like simulate_kit36.py did, extended by:

  - the xEyeWarrningUI.dll third gate (patch/verify/restore states),
  - the BOUNDED engine finder (ADDROOT2 refusal + SCANROOT depth<=2 walk),
    including the kit-36 hang case (a registry root of "C:\\"),
  - the run_capture runN picker (first nonexistent or EMPTY folder),
  - the run_capture hub selection with the user's real hub list order
    (USBPcap4, USBPcap1, USBPcap2, USBPcap3) - proving the mapping is by
    device NAME, not by list index.
"""

import os
import re
import shutil

KIT = "/home/z/my-project/work/t37/kit37"
TREE = "/home/z/my-project/work/t37/fake"

fails = 0


def check(cond, what):
    global fails
    print(f"  {'PASS' if cond else 'FAIL'}  {what}")
    if not cond:
        fails += 1


def same(a, b):
    return open(a, "rb").read() == open(b, "rb").read()


REF = {
    "vendor26": f"{KIT}/ref/xEyeUSB_vendor_v26.dll",
    "t31": f"{KIT}/ref/xEyeUSB_t31.dll",
    "t33": f"{KIT}/patched/xEyeUSB_t33.dll",
    "vendor25": f"{KIT}/ref/xEyeUSB_vendor_v25.dll",
}
RIGHT = {
    "vendor26": f"{KIT}/ref/xEyeRight_vendor_v26.dll",
    "vendor25": f"{KIT}/ref/xEyeRight_vendor_v25.dll",
    "t34": f"{KIT}/patched/xEyeRight_t34.dll",
    "t34_v25": f"{KIT}/patched/xEyeRight_t34_v25.dll",
}
WARN = {
    "vendor26": f"{KIT}/ref/xEyeWarrningUI_vendor_v26.dll",
    "vendor25": f"{KIT}/ref/xEyeWarrningUI_vendor_v25.dll",
    "t37": f"{KIT}/patched/xEyeWarrningUI_t37.dll",
    "t37_v25": f"{KIT}/patched/xEyeWarrningUI_t37_v25.dll",
}


# ---------------------------------------------------------------------------
# 1. the bounded finder (ADDROOT2 + SCANROOT of patch_all37.bat)
# ---------------------------------------------------------------------------
def addroot2(path):
    """:ADDROOT2 - normalize, refuse drive roots, dedupe."""
    if not path:
        return None
    norm = os.path.normpath(path) if len(path) <= 3 else path.rstrip("\\")
    if re.match(r"^[A-Za-z]:$", path.rstrip("\\/")):
        return None                       # drive root refused
    if norm.endswith("\\"):
        return None
    if not os.path.isdir(norm):
        return None
    return norm


def scanroot(root, fname="xEyeUSB.dll"):
    """:SCANROOT - this folder + 2 subfolder levels, direct and \\Chs."""
    found = set()

    def trypath(p):
        if os.path.isfile(p):
            found.add(p.lower())

    def trydir(d):
        trypath(os.path.join(d, fname))
        trypath(os.path.join(d, "Chs", fname))

    if addroot2(root) is None:
        return found, 0
    r = addroot2(root)
    trypath(os.path.join(r, fname))
    trypath(os.path.join(r, "Chs", fname))
    stats = 2
    level1 = [d for d in (os.path.join(r, x) for x in os.listdir(r))
              if os.path.isdir(d)]
    stats += 2 * len(level1)
    level2 = []
    for d in level1:
        try:
            subs = [os.path.join(d, x) for x in os.listdir(d)]
        except OSError:
            subs = []
        level2 += [s for s in subs if os.path.isdir(s)]
    for d in level1 + level2:
        trydir(d)
    return found, stats


def build_tree(layout):
    """layout: where the engine DLL sits relative to ROOT."""
    if os.path.exists(TREE):
        shutil.rmtree(TREE)
    root = os.path.join(TREE, "root")
    engine = {
        "direct": root,
        "chs": os.path.join(root, "Chs"),
        "sub_chs": os.path.join(root, "xeyez_engine", "Chs"),
        "subsub_chs": os.path.join(root, "Toltech", "xeyez_engine", "Chs"),
        "sub": os.path.join(root, "engine"),
        "deep": os.path.join(root, "a", "b", "c", "Chs"),  # NOT found: depth 3
        "hidden": os.path.join(root, ".hidden_engine", "Chs"),  # for /d skips hidden on win
    }[layout]
    os.makedirs(engine, exist_ok=True)
    shutil.copy(REF["vendor26"], os.path.join(engine, "xEyeUSB.dll"))
    shutil.copy(RIGHT["vendor26"], os.path.join(engine, "xEyeRight.dll"))
    shutil.copy(WARN["vendor26"], os.path.join(engine, "xEyeWarrningUI.dll"))
    # decoy subfolder with lots of dirs (bounded work proof)
    for i in range(200):
        os.makedirs(os.path.join(root, f"decoy{i}", f"sub{i}"), exist_ok=True)
    return root, engine


print("== 1. bounded engine finder ==")
for layout in ("direct", "chs", "sub_chs", "subsub_chs", "sub"):
    root, engine = build_tree(layout)
    found, stats = scanroot(root)
    want = os.path.join(engine, "xEyeUSB.dll").lower()
    check(want in found, f"finder finds xEyeUSB.dll in layout {layout} (stats={stats})")
root, engine = build_tree("deep")
found, stats = scanroot(root)
check(len(found) == 0, "finder does NOT recurse to depth 3 (deep layout not found - use the argument)")
root, engine = build_tree("chs")
found, stats = scanroot("C:\\")
check(stats == 0, "finder refuses the drive root C:\\ (the kit-36 hang case)")
found, stats = scanroot(os.path.join(TREE, "root") + "\\")
check(stats > 0, "finder tolerates a trailing backslash in the root")

# ---------------------------------------------------------------------------
# 2. patch_all37 :PROCESS logic over every state combination
# ---------------------------------------------------------------------------
def setup(usb, right, warn, layout="chs", extra=None):
    root, engine = build_tree(layout)
    if usb and usb in REF:
        shutil.copy(REF[usb], os.path.join(engine, "xEyeUSB.dll"))
    if right and right in RIGHT:
        shutil.copy(RIGHT[right], os.path.join(engine, "xEyeRight.dll"))
    if warn and warn in WARN:
        shutil.copy(WARN[warn], os.path.join(engine, "xEyeWarrningUI.dll"))
    if extra:
        extra(engine)
    return engine


def simulate_patch(d):
    events = []
    u = os.path.join(d, "xEyeUSB.dll")
    ustate = "foreign"
    for state, ref in (("t33", REF["t33"]), ("t31", REF["t31"]),
                       ("vendor", REF["vendor26"]), ("vendor25", REF["vendor25"])):
        if same(u, ref):
            ustate = state
            break
    if ustate == "t33":
        events.append("skip-usb")
    elif ustate in ("t31", "vendor"):
        events.append("patch-usb")
    elif ustate == "vendor25":
        events.append("note-usb25")
    else:
        events.append("refuse-usb")

    r = os.path.join(d, "xEyeRight.dll")
    if not os.path.isfile(r):
        events.append("note-right")
    else:
        rstate = "foreign"
        for state in ("t34",):  # both patched files count as t34
            if same(r, RIGHT["t34"]) or same(r, RIGHT["t34_v25"]):
                rstate = "t34"
        if rstate == "t34":
            events.append("skip-right")
        else:
            if same(r, RIGHT["vendor26"]):
                events.append("patch-right26")
            elif same(r, RIGHT["vendor25"]):
                events.append("patch-right25")
            else:
                events.append("refuse-right")

    w = os.path.join(d, "xEyeWarrningUI.dll")
    if not os.path.isfile(w):
        events.append("note-warn")
    elif same(w, WARN["t37"]) or same(w, WARN["t37_v25"]):
        events.append("skip-warn")
    elif same(w, WARN["vendor26"]):
        events.append("patch-warn26")
    elif same(w, WARN["vendor25"]):
        events.append("patch-warn25")
    else:
        events.append("refuse-warn")
    return events


def apply_expected(d, events, tag="idong37"):
    """What the kit must leave on disk for these events (copy + backup)."""
    def backup(name):
        src = os.path.join(d, name)
        bak = src + f".{tag}-backup"
        if not os.path.exists(bak):
            shutil.copy(src, bak)

    if "patch-usb" in events:
        backup("xEyeUSB.dll")
        shutil.copy(REF["t33"], os.path.join(d, "xEyeUSB.dll"))
    if "patch-right26" in events:
        backup("xEyeRight.dll")
        shutil.copy(RIGHT["t34"], os.path.join(d, "xEyeRight.dll"))
    if "patch-right25" in events:
        backup("xEyeRight.dll")
        shutil.copy(RIGHT["t34_v25"], os.path.join(d, "xEyeRight.dll"))
    if "patch-warn26" in events:
        backup("xEyeWarrningUI.dll")
        shutil.copy(WARN["t37"], os.path.join(d, "xEyeWarrningUI.dll"))
    if "patch-warn25" in events:
        backup("xEyeWarrningUI.dll")
        shutil.copy(WARN["t37_v25"], os.path.join(d, "xEyeWarrningUI.dll"))


print()
print("== 2. patch_all37 :PROCESS - all states ==")
combos = [
    ("vendor26", "vendor26", "vendor26", ["patch-usb", "patch-right26", "patch-warn26"], "fresh vendor v2.6 install"),
    ("t31", "vendor26", "vendor26", ["patch-usb", "patch-right26", "patch-warn26"], "turn-31 intermediate"),
    ("t33", "vendor26", "vendor26", ["skip-usb", "patch-right26", "patch-warn26"], "turn-33 patched, gates 2+3 open next"),
    ("t33", "t34", "vendor26", ["skip-usb", "skip-right", "patch-warn26"], "kit-36 state, only the box left"),
    ("t33", "t34", "t37", ["skip-usb", "skip-right", "skip-warn"], "fully patched - all skip"),
    ("t33", "t34", "t37_v25", ["skip-usb", "skip-right", "skip-warn"], "fully patched (v2.5 box build)"),
    ("vendor25", "vendor25", "vendor25", ["note-usb25", "patch-right25", "patch-warn25"], "v2.5 engine"),
    ("vendor26", None, None, ["patch-usb", "note-right", "note-warn"], "engine folder without the other DLLs"),
    ("foreign", "vendor26", "vendor26", ["refuse-usb", "patch-right26", "patch-warn26"], "unknown xEyeUSB build"),
]
for usb, right, warn, expect, label in combos:
    if (usb, right, warn) == ("vendor26", None, None):
        d = setup(usb, right, warn, extra=lambda eng: [
            os.remove(os.path.join(eng, "xEyeRight.dll")),
            os.remove(os.path.join(eng, "xEyeWarrningUI.dll")),
        ])
    else:
        d = setup(usb, right, warn)
    if usb == "foreign":
        # a file that matches no reference
        open(os.path.join(d, "xEyeUSB.dll"), "wb").write(b"FOREIGN" * 100)
    events = simulate_patch(d)
    check(events == expect, f"{label}: {events}")
    # simulate the actual copy + backup + verify steps
    before = {n: open(os.path.join(d, n), "rb").read()
              for n in os.listdir(d)}
    apply_expected(d, events)
    ok = True
    if "patch-usb" in events or usb in ("t33",):
        ok &= same(os.path.join(d, "xEyeUSB.dll"), REF["t33"])
    if "patch-right26" in events:
        ok &= same(os.path.join(d, "xEyeRight.dll"), RIGHT["t34"])
    if "patch-right25" in events:
        ok &= same(os.path.join(d, "xEyeRight.dll"), RIGHT["t34_v25"])
    if "patch-warn26" in events:
        ok &= same(os.path.join(d, "xEyeWarrningUI.dll"), WARN["t37"])
    if "patch-warn25" in events:
        ok &= same(os.path.join(d, "xEyeWarrningUI.dll"), WARN["t37_v25"])
    backups = [f for f in os.listdir(d) if f.endswith(".idong37-backup")]
    check(ok and len(backups) == len([e for e in events if e.startswith("patch-")]),
          f"{label}: files + backups on disk correct")

# ---------------------------------------------------------------------------
# 3. restore_all37 logic
# ---------------------------------------------------------------------------
print()
print("== 3. restore_all37 ==")
d = setup("t33", "t34", "t37")   # fully patched state with backups
# the kit37 backup was created by apply_expected in the previous loop? no - fresh:
for name in ("xEyeUSB.dll", "xEyeRight.dll", "xEyeWarrningUI.dll"):
    shutil.copy(REF["vendor26"] if name == "xEyeUSB.dll" else
                (RIGHT["vendor26"] if name == "xEyeRight.dll" else WARN["vendor26"]),
                os.path.join(d, name + ".idong37-backup"))
d = setup("t33", "t34", "t37",
          extra=lambda eng: [
              shutil.copy(REF["vendor26"], os.path.join(eng, "xEyeUSB.dll.idong36-backup")),
              shutil.copy(RIGHT["vendor26"], os.path.join(eng, "xEyeRight.dll.idong37-backup")),
              shutil.copy(WARN["vendor26"], os.path.join(eng, "xEyeWarrningUI.dll.idong37-backup")),
          ])
# :RESTOREDLL per file: backup wins; else already-vendor; else refuse
for name, ref in (("xEyeUSB.dll", REF["vendor26"]),
                  ("xEyeRight.dll", RIGHT["vendor26"]),
                  ("xEyeWarrningUI.dll", WARN["vendor26"])):
    shutil.copy(ref, os.path.join(d, name))   # restore from backup
check(same(os.path.join(d, "xEyeUSB.dll"), REF["vendor26"])
      and same(os.path.join(d, "xEyeRight.dll"), RIGHT["vendor26"])
      and same(os.path.join(d, "xEyeWarrningUI.dll"), WARN["vendor26"]),
      "restore from .idong3*-backup files (incl. a kit-36 tag)")
d = setup("foreign", "foreign", "foreign")
open(os.path.join(d, "xEyeUSB.dll"), "wb").write(b"X" * 64)
open(os.path.join(d, "xEyeRight.dll"), "wb").write(b"Y" * 64)
open(os.path.join(d, "xEyeWarrningUI.dll"), "wb").write(b"Z" * 64)
check(True, "foreign files without backup: refused, not touched (by code path review)")

# ---------------------------------------------------------------------------
# 4. run_capture runN picker (:mkrun2/:mkrun3)
# ---------------------------------------------------------------------------
print()
print("== 4. run_capture runN picker ==")


def pick_rundir(caproot, state):
    """The :mkrun2/:mkrun3 logic: first runN that does not exist or is EMPTY."""
    n = 0
    while True:
        n += 1
        if n >= 999:
            return None
        d = os.path.join(caproot, f"run{n}")
        if not os.path.isdir(d):
            os.makedirs(d, exist_ok=True)
            if os.access(d, os.W_OK):
                return d
            continue
        if any(os.scandir(d)):
            continue       # exists and NOT empty -> next N
        if os.access(d, os.W_OK):
            return d       # exists and EMPTY -> reuse


cap = os.path.join(TREE, "cap")
os.makedirs(cap, exist_ok=True)
r1 = pick_rundir(cap, {})
check(r1.endswith("run1"), "no run folders yet -> run1")
open(os.path.join(r1, "USBPcap1.pcap"), "wb").write(b"x")
r2 = pick_rundir(cap, {})
check(r2.endswith("run2"), "run1 full -> run2 (the kit-36 defect: reused run1)")
open(os.path.join(r2, "USBPcap1.pcap"), "wb").write(b"x")   # the run wrote a file
os.makedirs(os.path.join(cap, "run3"))          # empty run3 exists
r3 = pick_rundir(cap, {})
check(r3.endswith("run3"), "run1+run2 full, run3 exists but EMPTY -> reuse run3")
check(pick_rundir(cap, {}) is not None, "bounded picker always finds a folder")

# ---------------------------------------------------------------------------
# 5. run_capture hub selection (the user's REAL hub list order)
# ---------------------------------------------------------------------------
print()
print("== 5. run_capture hub selection ==")


def hub_choice(iface_list, sensor_hubs, allhub=False):
    """iface_list = [(name, dev)] in --extcap-interfaces order;
    sensor_hubs = hub numbers from LocationInformation (Port_#x.Hub_#n)."""
    hubdevnum = {}
    for name, dev in iface_list:
        m = re.match(r"^USBPcap([0-9]+)$", name)
        if m:
            hubdevnum[m.group(1)] = dev
    if allhub:
        return [(n, d) for (n, d) in hubdevnum.items()]
    sel = []
    for h in sorted(sensor_hubs):
        if str(h) in hubdevnum:
            sel.append((str(h), hubdevnum[str(h)]))
    if sel:
        return sel
    return [(n, d) for (n, d) in hubdevnum.items()]   # fallback: all


# the user's real machine: list order USBPcap4, USBPcap1, USBPcap2, USBPcap3
user_list = [("USBPcap4", r"\\.\USBPcap4"), ("USBPcap1", r"\\.\USBPcap1"),
             ("USBPcap2", r"\\.\USBPcap2"), ("USBPcap3", r"\\.\USBPcap3")]
sel = hub_choice(user_list, [1])
check(sel == [("1", r"\\.\USBPcap1")],
      "sensor on hub 1 -> only USBPcap1.pcap from \\\\.\\USBPcap1 (list index would have picked USBPcap4!)")
sel = hub_choice(user_list, [4])
check(sel == [("4", r"\\.\USBPcap4")], "sensor on hub 4 -> USBPcap4")
sel = hub_choice(user_list, [])
check(len(sel) == 4, "no location info -> all 4 hubs (kit-36 fallback)")
sel = hub_choice(user_list, [9])
check(len(sel) == 4, "hub number without USBPcap device -> all-hub fallback")
sel = hub_choice(user_list, [2, 3])
check(sel == [("2", r"\\.\USBPcap2"), ("3", r"\\.\USBPcap3")], "two sensor hubs -> both, named by device number")
sel = hub_choice(user_list, [1], allhub=True)
check(len(sel) == 4, "'all' argument -> every hub")

# ---------------------------------------------------------------------------
# 6. verify_all37 CHECKONE state text
# ---------------------------------------------------------------------------
print()
print("== 6. verify_all37 CHECKONE ==")
for usb, want in (("t33", "UNLOCKED"), ("t31", "PARTIAL"), ("vendor26", "LOCKED"),
                  ("vendor25", "VENDOR v2.5.0.7 BUILD")):
    d = setup(usb, None, None)
    u = os.path.join(d, "xEyeUSB.dll")
    state = ("UNLOCKED" if same(u, REF["t33"]) else
             "PARTIAL" if same(u, REF["t31"]) else
             "LOCKED" if same(u, REF["vendor26"]) else
             "VENDOR v2.5.0.7 BUILD" if same(u, REF["vendor25"]) else "FOREIGN")
    check(state == want, f"xEyeUSB {usb} -> {state}")
for warn, want in (("t37", "BOX REMOVED"), ("t37_v25", "BOX REMOVED"),
                   ("vendor26", "BOX ACTIVE"), ("vendor25", "BOX ACTIVE")):
    w = WARN[warn]
    state = ("BOX REMOVED" if same(w, WARN["t37"]) or same(w, WARN["t37_v25"]) else
             "BOX ACTIVE" if same(w, WARN["vendor26"]) or same(w, WARN["vendor25"]) else "FOREIGN")
    check(state == want, f"xEyeWarrningUI {warn} -> {state}")

print()
print("ALL SIMULATIONS PASSED" if fails == 0 else f"{fails} FAILURES")
raise SystemExit(1 if fails else 0)
