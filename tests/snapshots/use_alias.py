# `Use … As` renames the import for a pip package whose install name differs from
# its import name — you `pip install PyYAML`, but you `import yaml`. The package
# name (PyYAML) lands in requirements.txt; the alias (yaml) is what the generated
# code — and any inline `Python` block — actually imports and calls.
# 
# (Python-target example: PyYAML is a pip package, so it builds under `vbr py`.)

import yaml

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    doc: str = 'name: VBR\nkind: transpiler'
    # The aliased module is in scope for the inline block — no re-import needed.
    kind = yaml.safe_load(doc)["kind"]
    print(f"kind is {_vb(kind)}")


if __name__ == "__main__":
    main()
