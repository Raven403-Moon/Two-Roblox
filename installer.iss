; Two-Roblox Inno Setup Script
#define MyAppName "Two-Roblox"
#define MyAppVersion "1.1.0"
#define MyAppPublisher "Two-Roblox Team"
#define MyAppExeName "Two-Roblox.exe"

[Setup]
AppId={{7E1C5A8B-4B39-44F0-8772-9EBE8A7B2F62}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=dist
OutputBaseFilename=Two-Roblox-Setup-v{#MyAppVersion}
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\assets\icon.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

[Languages]
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "target\release\two_roblox_rs.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "tools\handle64.exe"; DestDir: "{app}\tools"; Flags: ignoreversion
Source: "assets\icon.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "assets\icon.png"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\icon.ico"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
