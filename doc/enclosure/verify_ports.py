"""Reopen both files; verify only the requested CAD changes and STL units."""
from pathlib import Path
import hashlib
import json
import struct

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT/'doc/enclosure/port_fit'
report = json.loads((OUT/'dimensions.json').read_text())
source = ROOT/report['source_file']
assert hashlib.sha256(source.read_bytes()).hexdigest() == report['source_sha256']


def records():
    result = {}
    for obj in bpy.context.scene.objects:
        if obj.type not in ('MESH', 'EMPTY'): continue
        data = {'vertices': [list(v.co) for v in obj.data.vertices],
                'faces': [list(p.vertices) for p in obj.data.polygons]} if obj.type == 'MESH' else {}
        result[obj.name] = {'mesh': hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest(),
                            'matrix': [list(row) for row in obj.matrix_basis],
                            'parent': obj.parent.name if obj.parent else None}
    return result


bpy.ops.wm.open_mainfile(filepath=str(source), use_scripts=False)
before = records()
bpy.ops.wm.open_mainfile(filepath=str(ROOT/report['output_file']), use_scripts=False)
after = records()
assert before.keys() == after.keys(), 'Original mesh objects must be retained'
cutters = set(report['openings_after_world_bounds_mm'])
moved_roots = ('ESP32-S3-Zero —', 'HEATSINK —')
for name, old in before.items():
    if name in cutters: continue
    new = after[name]
    assert new['mesh'] == old['mesh'], name
    # Basis transforms are authoritative even for hidden warning collections;
    # Blender need not evaluate their cached matrix_world after reopening.
    assert new['parent'] == old['parent'], name
    expected = [row[:] for row in old['matrix']]
    if name.startswith(moved_roots):
        expected[2][3] += report['esp_delta_z_mm']
    assert max(abs(a-b) for row_a, row_b in zip(expected, new['matrix'])
               for a, b in zip(row_a, row_b)) < .00001, name

for name, expected in report['openings_after_world_bounds_mm'].items():
    obj = bpy.data.objects[name]
    points = [obj.matrix_world @ Vector(v) for v in obj.bound_box]
    actual = [[min(v[i] for v in points), max(v[i] for v in points)] for i in range(3)]
    assert all(abs(a-b) < .00001 for x, y in zip(actual, expected) for a, b in zip(x, y)), name

for name, expected in report['exports'].items():
    data = (OUT/name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == expected['sha256'], name
    count = struct.unpack_from('<I', data, 80)[0]
    assert len(data) == 84 + count * 50, name
    points = []
    for index in range(count):
        row = struct.unpack_from('<12fH', data, 84 + 50 * index)
        points.extend([row[3:6], row[6:9], row[9:12]])
    size = [max(p[i] for p in points) - min(p[i] for p in points) for i in range(3)]
    assert all(abs(a-b) < .001 for a, b in zip(size, expected['dimensions_mm'])), name
    assert expected['nonmanifold_edges'] == 0 and expected['volume_mm3'] > 0, name
print('VERIFIED: source SHA; unchanged meshes/covers; ESP+USB+heatsink shift; 3 apertures; 4 closed STL exports in mm')
