// Source: https://github.com/lf-/nix-doc/blob/55599ec502e01e96673cbfcd2940b9dc1d392ec5/plugin/build.rs#L5C1-L27C2
trait AddPkg {
    fn add_pkg_config(&mut self, pkg: pkg_config::Library) -> &mut Self;
}
impl AddPkg for cc::Build {
    fn add_pkg_config(&mut self, pkg: pkg_config::Library) -> &mut Self {
        for p in pkg.include_paths.into_iter() {
            self.flag("-isystem").flag(p.to_str().unwrap());
        }
        for p in pkg.link_paths.into_iter() {
            self.flag(format!("-L{:?}", p));
        }
        for p in pkg.libs.into_iter() {
            self.flag(format!("-l{}", p));
        }
        for p in pkg.framework_paths.into_iter() {
            self.flag(format!("-F{:?}", p));
        }
        for p in pkg.frameworks.into_iter() {
            self.flag(format!("-framework {}", p));
        }
        self
    }
}

fn main() {
    let nix_expr = pkg_config::Config::new()
        .atleast_version("2.24")
        .probe("nix-expr")
        .unwrap();
    let nix_main = pkg_config::Config::new()
        .atleast_version("2.24")
        .probe("nix-main")
        .unwrap();
    let nix_store = pkg_config::Config::new()
        .atleast_version("2.24")
        .probe("nix-store")
        .unwrap();

    let nix_ver = nix_expr.version.clone();

    println!("cargo::rerun-if-changed=plugin.cpp");
    cc::Build::new()
        .file("plugin.cpp")
        .cpp(true)
        .opt_level(2)
        // .shared_flag(true)
        .std("c++20")
        .add_pkg_config(nix_expr)
        .add_pkg_config(nix_store)
        .add_pkg_config(nix_main)
        .define("BUILD_NIX_VERSION", Some(nix_ver.as_str()))
        .cargo_metadata(true)
        .link_lib_modifier("+whole-archive")
        .compile("plugin");
}
