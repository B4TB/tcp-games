{
  description = "A variety of games playable via netcat/telnet.";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        version = "0.1.0";
      in {
        packages = {
          nim = pkgs.stdenv.mkDerivation {
            pname = "nim";
            inherit version;

            dontBuild = true;
            dontUnpack = true;

            propagatedBuildInputs = with pkgs; [ python3 ];
            installPhase = ''
              install -Dm755 ${./examples/nim.py} $out/bin/nim
            '';
          };

          fermi = pkgs.stdenv.mkDerivation {
            pname = "fermi";
            inherit version;

            src = ./src/fermi;

            buildInputs = [ pkgs.ghc ];

            buildPhase = ''
              ghc -O2 -dynamic fermi.hs -o fermi
            '';

            installPhase = ''
              install -Dm755 ./fermi $out/bin/fermi
            '';
          };
        };
      }) // {
        nixosModules.nim = { pkgs, ... }: {
          systemd.services.nim = {
            description = "TCP Games: Nim";
            wantedBy = [ "multi-user.target" ];
            serviceConfig = {
              Type = "simple";
              ExecStart = "${self.packages.${pkgs.system}.nim}/bin/nim";
            };
          };

          networking.firewall.allowedTCPPorts = [ 5773 ];
        };

        nixosModules.fermi = { pkgs, ... }: {
          systemd.services.fermi = {
            description = "TCP Games: Fermi";
            wantedBy = [ "multi-user.target" ];
            serviceConfig = {
              Type = "simple";
              ExecStart = "${pkgs.socat}/bin/socat TCP-LISTEN:1337,reuseaddr,fork EXEC:${self.packages.${pkgs.system}.fermi}/bin/fermi";
            };
          };

          networking.firewall.allowedTCPPorts = [ 1337 ];
        };
      };
}
