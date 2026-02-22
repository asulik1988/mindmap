; Inno Setup script for Mindmap
; Build: iscc installer\mindmap-installer.iss

#define MyAppName "Mindmap"
#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif
#define MyAppPublisher "Adam Sulik"
#define MyAppURL "https://github.com/asulik1988/mindmap"
#define MyAppExeName "mindmap.exe"

[Setup]
AppId={{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\target\installer
OutputBaseFilename=mindmap-{#MyAppVersion}-setup
SetupIconFile=..\assets\mindmap.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "fileassoc"; Description: "Associate .mm files with {#MyAppName}"; GroupDescription: "File association:"; Flags: checkedonce

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\mindmap.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\mindmap.ico"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\mindmap.ico"; Tasks: desktopicon

[Registry]
; File association: .mm -> Mindmap
Root: HKCU; Subkey: "Software\Classes\.mm"; ValueType: string; ValueData: "Mindmap.Document"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\Mindmap.Document"; ValueType: string; ValueData: "FreeMind Mind Map"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\Mindmap.Document\DefaultIcon"; ValueType: string; ValueData: "{app}\mindmap.ico,0"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKCU; Subkey: "Software\Classes\Mindmap.Document\shell\open\command"; ValueType: string; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: fileassoc

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
