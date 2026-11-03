#!/usr/bin/env python3
"""Install credential checks in this checkout without replacing a user's hooks."""
from pathlib import Path
import shutil
import shlex
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
checker = root / "tools/check_public_files.py"
shell = shutil.which("sh")
if not shell:
    raise SystemExit("A POSIX shell is required to install Git hooks")
for repo in [root, *(root / name for name in ("App", "Bootloader", "Firmware"))]:
    # In a fresh non-recursive clone an empty submodule directory still lets Git
    # discover the parent repository. Never create an accidental App/.git there.
    discovered = subprocess.run(["git", "-C", str(repo), "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    if discovered.returncode or Path(discovered.stdout.strip()).resolve() != repo.resolve():
        print(f"Skipped {repo.name}: submodule is not initialized")
        continue
    hooks = Path(subprocess.check_output(["git", "-C", str(repo), "rev-parse", "--git-path", "hooks"], text=True).strip())
    if not hooks.is_absolute():
        hooks = repo / hooks
    hooks.mkdir(parents=True, exist_ok=True)
    for name in ("pre-commit", "pre-push"):
        hook = hooks / name
        if hook.exists() and "aosp-wear-credential-guard" not in hook.read_text(errors="replace"):
            print(f"Preserved existing {repo.name}/{name}; run tools/check_public_files.py manually")
            continue
        # Dynamic --show-toplevel also works when an existing hooksPath is shared by repositories.
        hook.write_text(f"#!{shell}\n# aosp-wear-credential-guard\nexec {shlex.quote(sys.executable)} {shlex.quote(str(checker))} --repo \"$(git rev-parse --show-toplevel)\"\n")
        hook.chmod(0o700)
        print(f"Installed {repo.name}/{name}")
