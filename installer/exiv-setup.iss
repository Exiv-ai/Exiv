; ============================================================
; Exiv - Windows Installer (Inno Setup)
; Extensible Intelligence Virtualization
;
; Build:
;   iscc.exe exiv-setup.iss /DAppVersion=0.1.0
;
; Prerequisites:
;   - Place exiv_system.exe in build/ directory
;   - icon.ico in assets/ directory (copied from dashboard/src-tauri/icons/)
; ============================================================

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif

[Setup]
AppId={{A7E3F2B1-5C4D-4A8E-B9F0-1D2E3F4A5B6C}
AppName=Exiv
AppVersion={#AppVersion}
AppVerName=Exiv {#AppVersion}
AppPublisher=Exiv Project
AppPublisherURL=https://github.com/Exiv-ai/Exiv
AppSupportURL=https://github.com/Exiv-ai/Exiv/issues
DefaultDirName={autopf}\Exiv
DefaultGroupName=Exiv
LicenseFile=..\LICENSE
OutputDir=output
OutputBaseFilename=exiv-setup-{#AppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\exiv_system.exe
WizardStyle=modern
DisableProgramGroupPage=yes
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Types]
Name: "full"; Description: "Full installation"
Name: "core"; Description: "Core only (no Python Bridge)"
Name: "custom"; Description: "Custom installation"; Flags: iscustom

[Components]
Name: "core"; Description: "Exiv Core System"; Types: full core custom; Flags: fixed
Name: "python"; Description: "Python Bridge (requires Python 3.9+)"; Types: full custom
Name: "dashboard"; Description: "Desktop Dashboard (Tauri)"; Types: full custom

[Files]
; Core binary
Source: "build\exiv_system.exe"; DestDir: "{app}"; Components: core; Flags: ignoreversion
; Configuration template
Source: "..\env.example"; DestDir: "{app}"; DestName: ".env.example"; Components: core; Flags: ignoreversion
; License
Source: "..\LICENSE"; DestDir: "{app}"; Components: core; Flags: ignoreversion
; Python bridge scripts
Source: "..\scripts\bridge_runtime.py"; DestDir: "{app}\scripts"; Components: python; Flags: ignoreversion
Source: "..\scripts\bridge_main.py"; DestDir: "{app}\scripts"; Components: python; Flags: ignoreversion
Source: "..\scripts\requirements.txt"; DestDir: "{app}\scripts"; Components: python; Flags: ignoreversion
; Uninstaller helper
Source: "..\uninstall.ps1"; DestDir: "{app}"; Components: core; Flags: ignoreversion

[Icons]
Name: "{group}\Exiv Dashboard"; Filename: "http://localhost:8081"; IconFilename: "{app}\exiv_system.exe"; Comment: "Open Exiv Dashboard"
Name: "{group}\Exiv Command Line"; Filename: "{cmd}"; Parameters: "/k cd /d ""{app}"""; IconFilename: "{app}\exiv_system.exe"; Comment: "Exiv CLI"
Name: "{group}\Uninstall Exiv"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Exiv Dashboard"; Filename: "http://localhost:8081"; IconFilename: "{app}\exiv_system.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional options:"
Name: "addtopath"; Description: "Add Exiv to system PATH"; GroupDescription: "Additional options:"; Flags: checkedonce
Name: "installservice"; Description: "Register as Windows Service (auto-start)"; GroupDescription: "Additional options:"

[Run]
; Run self-installer to set up directories, .env, and Python venv
Filename: "{app}\exiv_system.exe"; Parameters: "install --prefix ""{app}"" {code:GetInstallFlags}"; StatusMsg: "Configuring Exiv..."; Flags: runhidden waituntilterminated
; Optionally open dashboard after install
Filename: "http://localhost:8081"; Description: "Open Exiv Dashboard"; Flags: postinstall nowait shellexec skipifsilent unchecked

[UninstallRun]
; Stop service before uninstall
Filename: "sc.exe"; Parameters: "stop Exiv"; Flags: runhidden; RunOnceId: "StopService"
Filename: "sc.exe"; Parameters: "delete Exiv"; Flags: runhidden; RunOnceId: "DeleteService"

[Registry]
; Store install info for detection
Root: HKLM; Subkey: "SOFTWARE\Exiv"; ValueType: string; ValueName: "InstallPath"; ValueData: "{app}"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Exiv"; ValueType: string; ValueName: "Version"; ValueData: "{#AppVersion}"; Flags: uninsdeletekey

[Code]
// Build install flags based on task selections
function GetInstallFlags(Param: String): String;
var
  Flags: String;
begin
  Flags := '';
  if WizardIsTaskSelected('installservice') then
    Flags := Flags + ' --service';
  if not WizardIsComponentSelected('python') then
    Flags := Flags + ' --no-python';
  Result := Trim(Flags);
end;

// Add to PATH
procedure AddToPath();
var
  CurrentPath: String;
  InstallDir: String;
begin
  InstallDir := ExpandConstant('{app}');
  if RegQueryStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', CurrentPath) then
  begin
    if Pos(Uppercase(InstallDir), Uppercase(CurrentPath)) = 0 then
    begin
      CurrentPath := CurrentPath + ';' + InstallDir;
      RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', CurrentPath);
      Log('Added to PATH: ' + InstallDir);
    end;
  end;
end;

// Remove from PATH
procedure RemoveFromPath();
var
  CurrentPath: String;
  InstallDir: String;
  NewPath: String;
  P: Integer;
begin
  InstallDir := ExpandConstant('{app}');
  if RegQueryStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', CurrentPath) then
  begin
    P := Pos(';' + Uppercase(InstallDir), Uppercase(CurrentPath));
    if P > 0 then
    begin
      NewPath := Copy(CurrentPath, 1, P - 1) + Copy(CurrentPath, P + Length(InstallDir) + 1, MaxInt);
      RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', NewPath);
      Log('Removed from PATH: ' + InstallDir);
    end
    else begin
      P := Pos(Uppercase(InstallDir) + ';', Uppercase(CurrentPath));
      if P > 0 then
      begin
        NewPath := Copy(CurrentPath, 1, P - 1) + Copy(CurrentPath, P + Length(InstallDir) + 1, MaxInt);
        RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', NewPath);
        Log('Removed from PATH: ' + InstallDir);
      end;
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('addtopath') then
      AddToPath();
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveFromPath();
  end;
end;
