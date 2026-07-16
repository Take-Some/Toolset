# North Star Hasher

First-party Rust replacement for the legacy `hasher.exe` utility.

## Purpose

Hashes newline-separated name lists with a RAGE/JOAAT-compatible 32-bit string hash.

## Usage

```text
northstar-hasher [-stripext] [-literal] filename [hashcode]
```

- `-stripext` removes the final file extension before hashing.
- `-literal` hashes bytes exactly as written. Without it, ASCII bytes are lowercased before hashing.
- `hashcode` is an optional hexadecimal value. When present, only matching names are printed.

## Examples

```powershell
northstar-hasher names.txt
northstar-hasher -literal names.txt
northstar-hasher -stripext names.txt
northstar-hasher names.txt 0x1234abcd
```

## Output

```text
ATSTRINGHASH("adder",0x...)
ATLITERALSTRINGHASH("Adder",0x...)
```

## Notes

The old vendor `hasher.exe` did not accept `--help`. This replacement supports `--help`, `-h`, `/ ?` equivalent `/?`, `version`, `--version`, and `-V` for toolbelt consistency.
