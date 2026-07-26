; PiLPublisher Inno Setup Script
; 安装到用户 AppData，无需管理员权限

#define MyAppName "PiLPublisher"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "yxpil"
#define MyAppURL "https://github.com/yxpil/PILPublisher"
#define MyAppExeName "pil-publisher.exe"

[Setup]
AppId={{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={localappdata}\{#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=.\installer
OutputBaseFilename=PiLPublisher-Setup-0.1.0
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "src-tauri\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "启动 PiLPublisher"; Flags: nowait postinstall skipifsilent
