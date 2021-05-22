use crate::app::App;
use crate::app::ExternalMsg;
use crate::app::VERSION;
use crate::config::Config;
use anyhow::bail;
use anyhow::Result;
use mlua::Lua;
use mlua::LuaSerdeExt;
use std::fs;

const DEFAULT_LUA_SCRIPT: &str = include_str!("init.lua");
const UPGRADE_GUIDE_LINK: &str = "https://github.com/sayanarijit/xplr/wiki/Upgrade-Guide";

fn parse_version(version: &str) -> Result<(u16, u16, u16, Option<u16>)> {
    let mut configv = version.split('.');

    let major = configv.next().unwrap_or_default().parse::<u16>()?;
    let minor = configv.next().unwrap_or_default().parse::<u16>()?;
    let bugfix = configv
        .next()
        .and_then(|s| s.split('-').next())
        .unwrap_or_default()
        .parse::<u16>()?;

    let beta = configv.next().unwrap_or_default().parse::<u16>().ok();

    Ok((major, minor, bugfix, beta))
}

/// Check the config version and notify users.
pub fn check_version(version: &str, path: &str) -> Result<()> {
    // Until we're v1, let's ignore major versions
    let (rmajor, rminor, rbugfix, rbeta) = parse_version(VERSION)?;
    let (smajor, sminor, sbugfix, sbeta) = parse_version(version)?;

    if rmajor == smajor && rminor == sminor && rbugfix <= sbugfix && rbeta == sbeta {
        Ok(())
    } else {
        bail!(
            "incompatible script version in {}
                The script version is : {}
                Required version is   : {}
                Visit {}",
            path,
            version,
            VERSION.to_string(),
            UPGRADE_GUIDE_LINK,
        )
    }
}

fn resolve_fn_recursive<'lua, 'a>(
    table: &mlua::Table<'lua>,
    mut path: impl Iterator<Item = &'a str>,
) -> Result<mlua::Function<'lua>> {
    if let Some(nxt) = path.next() {
        match table.get(nxt)? {
            mlua::Value::Table(t) => resolve_fn_recursive(&t, path),
            mlua::Value::Function(f) => Ok(f),
            t => bail!("{:?} is not a function", t),
        }
    } else {
        bail!("Invalid path")
    }
}

/// This function resolves paths like `builtin.func_foo`, `custom.func_bar` into lua functions.
pub fn resolve_fn<'lua>(globals: &mlua::Table<'lua>, path: &str) -> Result<mlua::Function<'lua>> {
    let path = format!("xplr.fn.{}", path);
    resolve_fn_recursive(&globals, path.split('.'))
}

/// Used to initialize Lua globals
pub fn init(lua: &Lua) -> Result<Config> {
    let config = Config::default();
    let globals = lua.globals();

    let lua_xplr = lua.create_table()?;
    lua_xplr.set("config", lua.to_value(&config)?)?;

    let lua_xplr_fn = lua.create_table()?;
    let lua_xplr_fn_builtin = lua.create_table()?;
    let lua_xplr_fn_custom = lua.create_table()?;

    lua_xplr_fn.set("builtin", lua_xplr_fn_builtin)?;
    lua_xplr_fn.set("custom", lua_xplr_fn_custom)?;
    lua_xplr.set("fn", lua_xplr_fn)?;
    globals.set("xplr", lua_xplr)?;

    lua.load(DEFAULT_LUA_SCRIPT).set_name("init")?.exec()?;

    let lua_xplr: mlua::Table = globals.get("xplr")?;
    let config: Config = lua.from_value(lua_xplr.get("config")?)?;
    Ok(config)
}

/// Used to extend Lua globals
pub fn extend(lua: &Lua, path: &str) -> Result<Config> {
    let globals = lua.globals();

    let script = fs::read_to_string(path)?;

    lua.load(&script).set_name("init")?.exec()?;

    let version: String = match globals.get("version").and_then(|v| lua.from_value(v)) {
        Ok(v) => v,
        Err(_) => bail!("'version' must be defined globally in {}", path),
    };

    check_version(&version, path)?;

    let lua_xplr: mlua::Table = globals.get("xplr")?;

    let config: Config = lua.from_value(lua_xplr.get("config")?)?;
    Ok(config)
}

/// Used to extend Lua globals
pub fn call(lua: &Lua, func: &str, args: &App) -> Result<Vec<ExternalMsg>> {
    let func = resolve_fn(&lua.globals(), func)?;
    let args = lua.to_value(args)?;
    let msgs: mlua::Value = func.call((args,))?;
    let msgs: Vec<ExternalMsg> = lua.from_value(msgs)?;
    Ok(msgs)
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_compatibility() {
        assert!(check_version(VERSION, "foo path").is_ok());
        assert!(check_version("0.10.0", "foo path").is_ok());

        assert!(check_version("0.10.0-beta.6", "foo path").is_err());
        assert!(check_version("0.9.0", "foo path").is_err());
        assert!(check_version("0.11.0", "foo path").is_err());
        assert!(check_version("0.10.0-beta.5", "foo path").is_err());
        assert!(check_version("0.10.0-beta.7", "foo path").is_err());
        assert!(check_version("1.10.0-beta.6", "foo path").is_err());
    }
}
