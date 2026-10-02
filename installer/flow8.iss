; FLOW 8 PC Controller user-mode installer. DirectHCI is installed separately.
#ifndef Flow8BinaryDir
  #error Define Flow8BinaryDir to the Windows release binary directory.
#endif
#ifndef Flow8Version
  #error Define Flow8Version from Cargo package metadata.
#endif

[Setup]
AppId=Flow8PcController-Rust-Windows-x64
AppName=FLOW 8 PC Controller
AppVersion={#Flow8Version}
AppPublisher=FLOW 8 PC Controller contributors
DefaultDirName={userpf}\FLOW 8 PC Controller
DefaultGroupName=FLOW 8 PC Controller
DisableDirPage=no
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
OutputBaseFilename=FLOW-8-PC-Controller-{#Flow8Version}-windows-x64
Compression=lzma
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\flow8-gui.exe
SetupIconFile=..\apps\flow8-gui\assets\flow8.ico
LicenseFile=..\LICENSE.txt

[Files]
Source: "{#Flow8BinaryDir}\flow8-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.zh-CN.md"; DestDir: "{app}"; Flags: ignoreversion

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Icons]
Name: "{group}\FLOW 8 PC Controller"; Filename: "{app}\flow8-gui.exe"; IconFilename: "{app}\flow8-gui.exe"
Name: "{group}\Uninstall FLOW 8 PC Controller"; Filename: "{uninstallexe}"
Name: "{autodesktop}\FLOW 8 PC Controller"; Filename: "{app}\flow8-gui.exe"; IconFilename: "{app}\flow8-gui.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\flow8-gui.exe"; Description: "Launch FLOW 8 PC Controller"; Flags: postinstall nowait skipifsilent unchecked

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and
     not RegKeyExists(HKEY_LOCAL_MACHINE,
       'SYSTEM\CurrentControlSet\Services\DirectHCI') then begin
    Log('DirectHCI service is not installed; BLE connection will be unavailable.');
    if not WizardSilent then
      MsgBox('FLOW 8 PC Controller was installed. To connect on Windows, install the separate DirectHCI runtime and its supported controller driver setup. This installer does not manage Windows Bluetooth drivers.',
        mbInformation, MB_OK);
  end;
end;
