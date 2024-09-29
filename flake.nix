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

          images = pkgs.buildGoModule {
            pname = "images";
            inherit version;

            src = ./src/images;
            vendorHash = "sha256-cukmtsu303EagzwSIF/ptIfEmO9bzsT0Y1+SOiNa+6M=";
          };

          catlibrary = pkgs.callPackage ./src/catlibrary {};

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

        nixosModules.images = { pkgs, ... }: {
          systemd.services.images = {
            description = "TUI image viewer";
            wantedBy = [ "multi-user.target" ];
            serviceConfig = {
              Type = "simple";
              ExecStart = "${self.packages.${pkgs.system}.images}/bin/image-server-thing";
            };
          };

          networking.firewall.allowedTCPPorts = [ 5173 ];
        };

        nixosModules.catlibrary = { pkgs, ... }: {
          systemd.services.images = {
            description = "cat library";
            wantedBy = [ "multi-user.target" ];
            serviceConfig = {
              Type = "simple";
              ExecStart = "${self.packages.${pkgs.system}.catlibrary}/bin/cat-library";
            };
          };

          networking.firewall.allowedTCPPorts = [ 6868 ];
        };
      };
}
