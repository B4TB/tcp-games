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
        };
      }) // {
        nixosModules.nim = { nixpkgs, ... }: {
          systemd.services.nim = {
            description = "TCP Games: Nim";
            wantedBy = [ "multi-user.target" ];
            serviceConfig = {
              Type = "simple";
              ExecStart = "${self.packages.${nixpkgs.system}.nim}/bin/nim";
            };
          };

          networking.firewall.allowedTCPPorts = [ 5773 ];
        };
      };
}
