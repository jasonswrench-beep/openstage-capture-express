; SPDX-License-Identifier: Apache-2.0
; The installer of OpenStage Capture Express, made with Inno Setup (free to use for any purpose; see docs\installer.md).
; It is built by MAKE EXPRESS INSTALLER.bat (tools\make-installer.ps1), which gives these values with /D:
;   AppVersion  the program's version, read from Cargo.toml
;   StageDir    the folder holding the program (with the icon in it), its licence files and START-HERE.txt
;   WizardDir   the folder holding the setup wizard's pictures, which are drawn from the product icon
;
; The product icon is assets\icon\OpenCapture_Express.ico. It is used for the installer file itself, the pictures in the setup
; wizard, the program file, the shortcuts, and the program's entry in Windows' list of installed programs.
;
; It installs for the person using it (no administrator needed) and does not write to the registry except for the entry in the list
; of installed programs. Removing it never deletes guides, exports or settings.

#ifndef AppVersion
  #error AppVersion was not given. Build with MAKE EXPRESS INSTALLER.bat.
#endif
#ifndef StageDir
  #error StageDir was not given. Build with MAKE EXPRESS INSTALLER.bat.
#endif
#ifndef WizardDir
  #error WizardDir was not given. Build with MAKE EXPRESS INSTALLER.bat.
#endif

#define AppName "OpenStage Capture Express"
#define AppExe "openstage-capture-express.exe"
#define AppIcon "OpenCapture_Express.ico"

[Setup]
; The id is what Windows knows this program by: it must never change, so that a new version replaces the old one.
AppId={{5D821EED-6998-4D51-8960-02185FFA5D86}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Jason S. Wrench and the OpenStage Capture Express contributors
VersionInfoVersion={#AppVersion}.0
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} setup
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
DisableReadyPage=no
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19041
Compression=lzma2/max
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
WizardStyle=modern dynamic
; The product icon: the setup file's own icon, the entry in the list of installed programs, and the wizard's pictures.
SetupIconFile=..\assets\icon\{#AppIcon}
UninstallDisplayIcon={app}\{#AppIcon}
UninstallDisplayName={#AppName}
WizardImageFile={#WizardDir}\tall-164.png,{#WizardDir}\tall-246.png,{#WizardDir}\tall-328.png
WizardSmallImageFile={#WizardDir}\small-55.png,{#WizardDir}\small-83.png,{#WizardDir}\small-110.png
#ifdef SignTool
; A signed build (make-installer.ps1 -Sign): the compiler signs the setup file and its uninstaller with the sign tool named on its
; command line, which signs with Azure Artifact Signing and timestamps every signature (tools\sign_windows.py).
SignTool={#SignTool}
SignedUninstaller=yes
SignToolRetryCount=3
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
#ifdef SignTool
WelcomeLabel2=This will install [name/ver] on your computer.%n%nIt turns what you do on screen into a step-by-step guide. It has no account and makes no network connection.%n%nIt is recommended that you close other programs before continuing.
#else
WelcomeLabel2=This will install [name/ver] on your computer.%n%nIt turns what you do on screen into a step-by-step guide. It has no account and makes no network connection.%n%nThis version is not signed yet, so Windows may have warned you before starting this setup.%n%nIt is recommended that you close other programs before continuing.
#endif

[CustomMessages]
english.UninstallKept=Your guides, exports and settings were not deleted. They are in your Documents folder (in OpenStage Express) and in the AppData folder (in OpenStage Capture Express).

[Tasks]
Name: "desktopicon"; Description: "Put a shortcut on the &desktop"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#StageDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\NOTICE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\THIRD-PARTY.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\START-HERE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icon\{#AppIcon}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\{#AppIcon}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\{#AppIcon}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "Open {#AppName} now"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and not UninstallSilent then
    MsgBox(ExpandConstant('{cm:UninstallKept}'), mbInformation, MB_OK);
end;
