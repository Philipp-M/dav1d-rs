use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;

mod build {
    use super::*;
    use std::path::Path;
    use std::process::{Command, Stdio};

    const REPO: &str = "https://code.videolan.org/videolan/dav1d.git";

    macro_rules! runner {
        ($cmd:expr, $($arg:expr),*) => {
            Command::new($cmd)
                $(.arg($arg))*
                .stderr(Stdio::inherit())
                .output()
                .expect(concat!($cmd, " failed"));

        };
    }

    pub fn build_from_src(
        lib: &str,
        version: &str,
    ) -> Result<system_deps::Library, system_deps::BuildInternalClosureError> {
        let build_dir = "build";
        let release_dir = "release";

        let source = PathBuf::from(env::var("OUT_DIR").unwrap()).join("dav1d");
        let build_path = source.join(build_dir);
        let release_path = source.join(release_dir);

        if !Path::new(&source.join(".git")).exists() {
            runner!("git", "clone", "--depth", "1", REPO, &source);
        } else {
            runner!("git", "-C", source.to_str().unwrap(), "pull");
        }

        let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

        let use_cross_file =
            |path: &PathBuf| Some(format!("--cross-file={}", path.to_str().unwrap()));

        let cross_file = if target_arch == "aarch64" && target_os == "android" {
            use_cross_file(&source.join("package/crossfiles/aarch64-android.meson"))
        } else if target_arch == "arm" && target_os == "android" {
            use_cross_file(&source.join("package/crossfiles/arm-android.meson"))
        } else if target_os == "ios" {
            let mut hasher = DefaultHasher::new();
            let sdk = if target_arch == "x86_64" {
                "iphonesimulator" // TODO not working as expected yet thus the duplicate 'file_content' below
            } else {
                "iphoneos"
            };
            let xc_find = |program| -> String {
                String::from_utf8_lossy(&runner!("xcrun", "--sdk", sdk, "--find", program).stdout)
                    .trim()
                    .into()
            };
            let sysroot: String =
                String::from_utf8_lossy(&runner!("xcrun", "--sdk", sdk, "--show-sdk-path").stdout)
                    .trim()
                    .into();
            let platform_path: String = String::from_utf8_lossy(
                &runner!("xcrun", "--sdk", sdk, "--show-sdk-platform-path").stdout,
            )
            .trim()
            .into();
            let file_content = if target_arch == "x86_64" {
                r#"
[binaries]
c = 'clang'
cpp = 'clang++'
ar = 'ar'
strip = 'strip'
pkgconfig = 'pkg-config'

[built-in options]
# b_bitcode = true # TODO support this
c_args = ['-arch', 'x86_64']
c_link_args = ['-arch', 'x86_64']
cpp_args = ['-arch', 'x86_64']
cpp_link_args = ['-arch', 'x86_64']

[properties]
has_function_printf = true
has_function_hfkerhisadf = false
needs_exe_wrapper = true

[host_machine]
system = 'darwin'
cpu_family = 'x86_64'
endian = 'little'

cpu = 'x86_64'"#
                    .into()
            } else {
                format!(
                    r#"
[binaries]
c = '{clang}'
cpp = '{clang_cpp}'
ar = '{ar}'
strip = '{strip}'
pkgconfig = 'pkg-config'

[built-in options]
root = '{platform_path}/Developer'
b_bitcode = true
c_args = ['-mios-version-min=9.0', '-arch', '{arch}', '-isysroot', '{sysroot}', '-Werror=partial-availability', '-fno-stack-check']
c_link_args = ['-Wl,-ios_version_min,9.0', '-mios-version-min=9.0', '-arch', '{arch}', '-L{sysroot}/usr/lib/']
cpp_args = ['-mios-version-min=9.0', '-arch', '{arch}', '-isysroot', '{sysroot}', '-Werror=partial-availability', '-fno-stack-check']
cpp_link_args = ['-Wl,-ios_version_min,9.0', '-mios-version-min=9.0', '-arch', '{arch}', '-L{sysroot}/usr/lib/']

[properties]
has_function_printf = true
has_function_hfkerhisadf = false
needs_exe_wrapper = true

[host_machine]
system = 'darwin'
cpu_family = 'arm'
endian = 'little'

cpu = '{arch}'"#,
                    clang = xc_find("clang"),
                    clang_cpp = xc_find("clang++"),
                    ar = xc_find("ar"),
                    strip = xc_find("strip"),
                    sysroot = sysroot,
                    platform_path = platform_path,
                    arch = if target_arch == "aarch64" {
                        "arm64"
                    } else {
                        &target_arch
                    }
                )
            };
            file_content.hash(&mut hasher);

            let filename = source.join(format!(
                "package/crossfiles/{arch}-ios-{hash}.meson",
                arch = target_arch,
                hash = hasher.finish()
            ));
            std::fs::write(&filename, file_content).expect("Couldn't write meson crossfile");
            use_cross_file(&filename)
        } else if target_os == "windows" && target_arch == "x86_64" {
            use_cross_file(&source.join("package/crossfiles/x86_64-w64-mingw32.meson"))
        } else if target_os == "windows" && target_arch == "x86" {
            use_cross_file(&source.join("package/crossfiles/i686-w64-mingw32.meson"))
        }
        else {
            None
        };

        if let Some(file) = cross_file {
            eprintln!("cross file: {}", file);
            runner!(
                "meson",
                "setup",
                file,
                "-Ddefault_library=static",
                "--prefix",
                release_path.to_str().unwrap(),
                build_path.to_str().unwrap(),
                source.to_str().unwrap()
            );
        } else {
            runner!(
                "meson",
                "setup",
                "-Ddefault_library=static",
                "--prefix",
                release_path.to_str().unwrap(),
                build_path.to_str().unwrap(),
                source.to_str().unwrap()
            );
        }
        runner!("ninja", "-C", build_path.to_str().unwrap());
        runner!("meson", "install", "-C", build_path.to_str().unwrap());

        let pkg_dir = build_path.join("meson-private");
        system_deps::Library::from_internal_pkg_config(&pkg_dir, lib, version)
    }
}

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    std::env::set_var("SYSTEM_DEPS_DAV1D_BUILD_INTERNAL", "always");
    if target_os == "ios" || target_os == "android" {
        std::env::set_var("PKG_CONFIG_ALLOW_CROSS", "1");
    }
    let libs = system_deps::Config::new()
        .add_build_internal("dav1d", |lib, version| build::build_from_src(lib, version))
        .probe()
        .unwrap();

    let libs = libs.get_by_name("dav1d").unwrap();

    let headers = libs.include_paths.clone();

    let mut builder = bindgen::builder()
        .blocklist_type("max_align_t")
        .size_t_is_usize(true)
        .header("data/dav1d.h");

    for header in headers {
        builder = builder.clang_arg("-I").clang_arg(header.to_str().unwrap());
    }

    // Manually fix the comment so rustdoc won't try to pick them
    let s = builder
        .generate()
        .unwrap()
        .to_string()
        .replace("/**", "/*")
        .replace("/*!", "/*");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut file = File::create(out_path.join("dav1d.rs")).unwrap();

    let _ = file.write(s.as_bytes());
}
