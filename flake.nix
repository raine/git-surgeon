{
  description = "Surgical git hunk control for AI agents";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.stdenv.mkDerivation {
            pname = cargoToml.package.name;
            version = cargoToml.package.version;

            src = ./.;

            nativeBuildInputs = with pkgs; [
              cargo
              rustc
            ];

            buildPhase = ''
              runHook preBuild
              export CARGO_HOME="$TMPDIR/cargo-home"
              cargo build --release --locked
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              install -Dm755 target/release/git-surgeon "$out/bin/git-surgeon"
              runHook postInstall
            '';

            meta = with pkgs.lib; {
              description = "Surgical git hunk control for AI agents";
              homepage = "https://github.com/raine/git-surgeon";
              license = licenses.mit;
              mainProgram = "git-surgeon";
            };
          };
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/git-surgeon";
        };
      });

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo
              rustc
              rust-analyzer
              rustfmt
              clippy
            ];

            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          };
        }
      );
    };
}
