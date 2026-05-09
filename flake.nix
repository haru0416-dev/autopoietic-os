{
  description = "Autopoietic OS research seed";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    home-manager.url = "github:nix-community/home-manager";
    home-manager.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, home-manager, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          autopoietic-tools = pkgs.rustPlatform.buildRustPackage {
            pname = "autopoietic-tools";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        in
        {
          inherit autopoietic-tools;
          default = autopoietic-tools;
        }
        // nixpkgs.lib.optionalAttrs (system == "x86_64-linux") {
          iso = self.nixosConfigurations.iso.config.system.build.isoImage;
        });

      apps = forAllSystems (system: {
        os-introspect = {
          type = "app";
          program = "${self.packages.${system}.autopoietic-tools}/bin/os-introspect";
          meta.description = "Emit Autopoietic OS self-state JSON";
        };
        mutation-journal = {
          type = "app";
          program = "${self.packages.${system}.autopoietic-tools}/bin/mutation-journal";
          meta.description = "Append Autopoietic OS mutation and effect journal entries";
        };
        mutation-runner = {
          type = "app";
          program = "${self.packages.${system}.autopoietic-tools}/bin/mutation-runner";
          meta.description = "Verify Autopoietic OS mutation proposals offline";
        };
        default = self.apps.${system}.os-introspect;
      });

      devShells = forAllSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = [
              pkgs.git
              pkgs.jq
              pkgs.rustc
              pkgs.cargo
              self.packages.${system}.autopoietic-tools
            ];
          };
        });

      checks = forAllSystems (system:
        { }
        // nixpkgs.lib.optionalAttrs (system == "x86_64-linux") (
          let
            isoTests = import ./tests/vm/iso-boot.nix {
              inherit nixpkgs system;
              autopoieticModule = self.nixosModules.autopoietic-core;
              autopoieticTools = self.packages.${system}.autopoietic-tools;
              productionIso = self.nixosConfigurations.iso.config.system.build.isoImage;
            };
          in
          {
            iso-boot-basic = isoTests.boot-basic;
            iso-observe-only = isoTests.observe-only;
            iso-tools = isoTests.tools;
            iso-uefi-boot = isoTests.uefi-boot;
            iso-production-boot-console = isoTests.production-boot-console;
            iso-production-uefi-boot-console = isoTests.production-uefi-boot-console;
            iso-boot = isoTests.boot-basic;
          }
        ));

      nixosModules.autopoietic-core = import ./modules/core;

      nixosConfigurations.aion = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          ./hosts/aion/configuration.nix
          home-manager.nixosModules.home-manager
          self.nixosModules.autopoietic-core
        ];
      };

      nixosConfigurations.iso = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = {
          autopoieticTools = self.packages.x86_64-linux.autopoietic-tools;
        };
        modules = [
          (nixpkgs + "/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix")
          ./hosts/iso/configuration.nix
          self.nixosModules.autopoietic-core
        ];
      };
    };
}
