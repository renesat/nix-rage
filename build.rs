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

struct NixVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: Option<u64>,
    pub pre: Option<String>,
}

impl NixVersion {
    fn from_str(version: &str) -> anyhow::Result<Self> {
        let re = regex::Regex::new(r"(\d+).(\d+)(.(\d+)|)(pre(.+)|)$")?;
        let caps = re
            .captures(version)
            .ok_or(anyhow::anyhow!("Fail to parse nix version!"))?;
        let major = caps.get(1).unwrap().as_str().parse().unwrap();
        let minor = caps.get(2).unwrap().as_str().parse().unwrap();
        let patch = caps.get(4).map(|x| x.as_str().parse().unwrap());
        let pre = caps.get(6).map(|x| x.as_str().to_string());
        Ok(Self {
            major,
            minor,
            patch,
            pre,
        })
    }
}

impl std::fmt::Display for NixVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}{}{}",
            self.major,
            self.minor,
            self.patch.map(|x| format!(".{x}")).unwrap_or_default(),
            self.pre
                .as_ref()
                .map(|x| format!("pre{x}"))
                .unwrap_or_default()
        )
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

    let nix_ver = NixVersion::from_str(&nix_expr.version).unwrap();
    let nix_major_ver = nix_ver.major.to_string();
    let nix_minor_ver = nix_ver.minor.to_string();
    let nix_patch_ver = nix_ver.patch.unwrap_or_default().to_string();

    println!("cargo::rerun-if-changed=plugin.cpp");
    cc::Build::new()
        .file("plugin.cpp")
        .cpp(true)
        .opt_level(2)
        .shared_flag(true)
        .std("c++20")
        .add_pkg_config(nix_expr)
        .add_pkg_config(nix_store)
        .add_pkg_config(nix_main)
        .define("NIX_MAJOR_VERSION", Some(nix_major_ver).as_deref())
        .define("NIX_MINOR_VERSION", Some(nix_minor_ver).as_deref())
        .define("NIX_PATCH_VERSION", Some(nix_patch_ver).as_deref())
        .define("NIX_VERSION", Some(nix_ver.to_string()).as_deref())
        .cargo_metadata(true)
        .link_lib_modifier("+whole-archive")
        .compile("plugin");
}
