{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
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
        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain toolchain;
        commonArgs = {
          src = lib.cleanSourceWith {
            src = lib.cleanSource ./.;
            filter = name: type: (craneLib.filterCargoSources name type) || (lib.hasSuffix ".cpp" name);
          };
          nativeBuildInputs = [pkgs.pkg-config];
          buildInputs = [
            pkgs.nix
            pkgs.boost
          ];
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
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
            cargo-machete = {
              enable = true;
              name = "cargo-machete";
              description = "Remove unused Rust dependencies with this one weird trick!";
              language = "rust";
              pass_filenames = false;
              entry = lib.getExe pkgs.cargo-machete;
            };
            zizmor = {
              name = "zizmor";
              description = "Find security issues in GitHub Actions CI/CD setups";
              language = "python";
              types = ["yaml"];
              files = "(\.github/workflows/.*)|(action\.ya?ml)$";
              require_serial = true;
              entry = lib.getExe pkgs.zizmor;
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
              print("Generate key...")
              machine1.execute("${pkgs.age}/bin/age-keygen -o /tmp/key")

              print("Test `importAge`...")
              machine1.execute(
                "echo '{ a = \"SECRET\";}' | ${pkgs.age}/bin/age -e -i /tmp/key > /tmp/data.age"
              )
              assert machine1.execute(
                "nix eval --raw --expr '(builtins.importAge [ /tmp/key ] /tmp/data.age {cache=false;}).a'"
              )[1] == "SECRET", "Import file error"

              print("Test `readAgeFile`...")
              machine1.execute(
                "echo 'SECRET' | ${pkgs.age}/bin/age -e -i /tmp/key > /tmp/data.age"
              )
              assert machine1.execute(
                "nix eval --raw --expr 'builtins.readAgeFile [ /tmp/key ] /tmp/data.age {cache=false;}'"
              )[1].strip() == "SECRET", "Read file error"
            '';
          };
        };

        devShells.default = craneLib.devShell {
          packages =
            [
              pkgs.bacon
              pkgs.just
              pkgs.cargo-watch
              pkgs.nix-output-monitor
              config.treefmt.build.wrapper
              pkgs.git-cliff
            ]
            ++ self'.packages.default.buildInputs
            ++ self'.packages.default.nativeBuildInputs;
          shellHook = config.pre-commit.installationScript;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath self'.packages.default.buildInputs;
        };

        packages = {
          nix-rage = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              doCheck = false;
            }
          );
          default = self'.packages.nix-rage;
        };
      };
    };
}
