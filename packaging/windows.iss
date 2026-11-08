#ifndef PayloadDir
  #define PayloadDir SourcePath + "..\dist\disk-space-tui-x86_64-pc-windows-msvc"
#endif
#ifndef ReleaseDir
  #define ReleaseDir SourcePath + "..\dist"
#endif
[Setup]
AppId={{5C390130-6082-463F-8B3E-64457D336852}
AppName=disk-space-tui
AppVersion=0.2.0
AppPublisher=Daniele Gabrovec
AppPublisherURL=https://github.com/danielegabrovec/disk-space-tui
DefaultDirName={localappdata}\Programs\disk-space-tui
DefaultGroupName=disk-space-tui
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir={#ReleaseDir}
OutputBaseFilename=disk-space-tui-0.2.0-Windows-x64-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
UninstallDisplayIcon={app}\disk-space-tui.exe
[Languages]
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
[Tasks]
Name: "desktopicon"; Description: "Crea un collegamento sul desktop"; Flags: unchecked
[Files]
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{group}\disk-space-tui"; Filename: "{app}\disk-space-tui.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\disk-space-tui"; Filename: "{app}\disk-space-tui.exe"; WorkingDir: "{app}"; Tasks: desktopicon
