{ lib, rustPlatform }: rustPlatform.buildRustPackage {
  pname = "catlibrary";
  version = "0.1.0";
  src = ./.;

  cargoHash = "sha256-NKOc5+79GO4PB408EyRGGRFzbwoZJmSAiPm9wYf90so=";
} 
