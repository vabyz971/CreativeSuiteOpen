{
  description = "Environnement dev - Suite créative";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }: {
    devShells.x86_64-linux.default = let
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
    in pkgs.mkShell {
      buildInputs = with pkgs; [
        cargo
        rustc
        rustfmt
        clippy
        rust-analyzer
        # Dépendances requises par wgpu/iced sur Linux
        pkg-config
        vulkan-loader
        libxkbcommon
        wayland
      ];
      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [ vulkan-loader libxkbcommon wayland ]);
    };
  };
}
