{
  lib,
  pkg-config,
  rustPlatform,
  stdenv,
  systemd,
}:

rustPlatform.buildRustPackage {
  pname = "sftool";
  version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).workspace.package.version;
  src = ../.;
  cargoLock = {
    lockFile = ../Cargo.lock;
  };
  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
  ]
  ++ lib.optionals stdenv.hostPlatform.isLinux [
    systemd # libudev-sys
  ];

  meta = {
    description = "Sftool is a download tool for the SiFli family of chips";
    homepage = "https://github.com/OpenSiFli/sftool";
    license = lib.licenses.asl20;
    maintainers = with lib.maintainers; [ ];
    mainProgram = "sftool";
  };
}
