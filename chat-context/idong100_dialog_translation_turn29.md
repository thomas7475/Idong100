# The "problem" dialog after driver install - turn 29 - 2026-11-25

User's screenshot (win7-afterdriver.install.png) shows a garbled (mojibake)
dialog right after the driver install, and asks: is this a problem, can we fix
it, or is another software version needed?

## What the dialog actually says

The garbling is just GBK Chinese rendered on a German/English Windows 7
(system codepage 1252). The exact string was found in the vendor binaries
(xEyeUSB.dll, file offset 0x16060 in v2.6.0.6, also present in v2.5.0.7):

> Title: idong感应器引擎 ("idong sensor engine")
> Body: 您购买的是爱动体感运动机(大众版)，若要升级到爱动体感运动机(运动王PC版)，
> 请拨打咨询热线400-1818-520。

Translation: "The i-dong motion-sensing console you purchased is the Public
Edition (大众版). To upgrade to the Sports King PC Edition (运动王PC版),
please call the hotline 400-1818-520."

## Verdict: it is an advertisement, not an error

Code-level proof (xEyeUSB.dll 2.6.0.6, .text):

- The dialog function (file offset 0x76d0) does
  `FindWindowA("xEyezEngine2009")` then `MessageBoxA(...)` with the ad text,
  caption "idong感应器引擎".
- It is called from the device-init state-machine step (file offset 0x6500,
  referenced from the init table at 0x16140). The code AFTER the dialog call
  loops straight back into the init sequence - the dialog does not abort, does
  not set an error, does not gate the camera path.
- The same string and the same logic exist in xEyeUSB.dll v2.5.0.7 (string at
  file offset 0x86BB region) - no vendor PC version is free of it.
- The capture of turn 28 independently proves the app continued normally after
  the dialog: the runtime protocol (0x0701 image requests, 0x0d00/0x0f04
  polls) ran for the rest of the session.

So: nothing is broken. The vendor software shows this ad whenever the sensor
initializes on a "Public Edition" install (the edition is derived from the
installed lobby software via registry `SOFTWARE\TOLTech` + Location ->
lobby\ConfigManagement.dll!GetLobbyVer, model letters DSHYK mapping to
edition codes). The user's Win7 test machine is running the 大众版 lobby.

## Can we fix it / do you need another version?

- **You do NOT need another software version** - v2.5.0.7 shows the same ad.
- **Nothing to "fix" functionally** - the driver path is unaffected. Press OK
  and continue; the app keeps working (as the USB capture proves).
- The mojibake (garbled characters) is cosmetic: the app is GBK, the Win7
  system locale is Western. If you want it readable, set the system locale for
  non-Unicode programs to Chinese (Simplified) - Control Panel > Region >
  Administrative > Change system locale. Not required for our work.
- The ad is shown by the VENDOR PC software (xeyez.exe / xEyeUSB.dll), not by
  our driver and not by the sensor firmware. Our Rust driver will never show
  it.

## Consequence for the capture plan

The dialog was NOT why the capture window lacked the DSP/FPGA upload and the
isochronous camera stream. The vendor app simply had not proceeded past its
init/polling stage within the 108 s recorded. The "full vendor session until
the camera image shows" capture is still the one to take next - when the
camera image actually displays, press OK on the ad (it appears at device init)
and keep the session running.

## Where the strings live (for the record)

| Item | Location in xEyeUSB.dll 2.6.0.6 |
| --- | --- |
| Ad text | .rdata VA 0x10016060 (100 B GBK), duplicate at 0x100160d8 |
| Caption | 0x1001604c / 0x100160c8 "idong感应器引擎" |
| Dialog function | .text file off 0x76d0 (FindWindowA + MessageBoxA via IAT 0x15324-0x15350) |
| Called from | init step 0x6500; dialog call at file off 0x67ff; code continues after it |
| Edition source | registry `SOFTWARE\TOLTech\Location` -> `lobby\ConfigManagement.dll` `GetLobbyVer`, letters DSHYK -> edition codes |
| Same in 2.5.0.7 | ad string present, same FindWindow/MessageBox pattern |
