{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} {
      systems = import inputs.systems;
      imports = [
        inputs.pre-commit-hooks.flakeModule
        inputs.treefmt-nix.flakeModule
      ];
      perSystem = {
        system,
        config,
        self',
        pkgs,
        lib,
        ...
      }: let
        toolchain = pkgs.fenix.stable.withComponents [
          "cargo"
          "clippy"
          "rust-src"
          "rustc"
          "rustfmt"
        ];
      in {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [
            inputs.fenix.overlays.default
          ];
        };

        treefmt = {
          programs = {
            rustfmt = {
              enable = true;
              package = toolchain;
            };
            clang-format.enable = true;
            alejandra.enable = true;
            taplo.enable = true;
            yamlfmt = {
              enable = true;
              settings = {
                formatter = {
                  include_document_start = true;
                  pad_line_comments = 2;
                };
              };
            };
          };
        };

        pre-commit.settings = {
          settings = {
            rust.check.cargoDeps = pkgs.rustPlatform.importCargoLock {lockFile = ./Cargo.lock;};
          };
          hooks = {
            yamllint.enable = true;
            treefmt = {
              enable = true;
              package = config.treefmt.build.wrapper;
            };
            clippy = {
              enable = true;
              packageOverrides.cargo = toolchain;
              packageOverrides.clippy = toolchain;
              extraPackages = self'.packages.default.buildInputs ++ self'.packages.default.nativeBuildInputs;
            };

            deadnix = {
              enable = true;
              args = ["--edit"];
            };
            statix = {
              enable = true;
              settings = {
                format = "stderr";
              };
            };
            nil.enable = true;
            ripsecrets.enable = true;
          };
        };

        checks = {
          testPlugin = pkgs.testers.runNixOSTest {
            name = "testPlugin";
            nodes.machine1 = {
              nix.extraOptions = ''
                plugin-files = ${self'.packages.default}/lib/libnix_rage.so
                experimental-features = nix-command
              '';
            };
            testScript = ''
              machine1.start()
              machine1.execute("${pkgs.age}/bin/age-keygen -o /tmp/key")

              # Test `importAge`
              machine1.execute(
                "echo '{ a = \"SECRET\";}' | ${pkgs.age}/bin/age -e -i /tmp/key > /tmp/data.age"
              )
              assert machine1.execute(
                "nix eval --raw --expr '(builtins.importAge [ /tmp/key ] /tmp/data.age).a'"
              )[1] == "SECRET", "Import file error"

              # Test `readAgeFile`
              machine1.execute(
                "echo 'SECRET' | ${pkgs.age}/bin/age -e -i /tmp/key > /tmp/data.age"
              )
              assert machine1.execute(
                "nix eval --raw --expr 'builtins.readAgeFile [ /tmp/key ] /tmp/data.age'"
              )[1].strip() == "SECRET", "Read file error"
            '';
          };
        };

        devShells.default = pkgs.mkShell {
          packages = [
            toolchain
            pkgs.rust-analyzer
            pkgs.bacon
            pkgs.just
            pkgs.cargo-watch
            pkgs.nix-output-monitor

            # Deps
            self'.packages.default.buildInputs
            self'.packages.default.nativeBuildInputs
          ];
          shellHook = config.pre-commit.installationScript;
        };

        packages.default =
          (inputs.naersk.lib.${system}.override {
            cargo = toolchain;
            rustc = toolchain;
          })
          .buildPackage
          {
            src = ./.;
            copyLibs = true;
            copyBins = false;
            nativeBuildInputs = [pkgs.pkg-config];
            buildInputs = [
              pkgs.nix
              pkgs.boost
            ];
          };
      };
    };
}
