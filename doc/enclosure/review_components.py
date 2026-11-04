"""Review the user's edited assembly without rebuilding its shell or deleted parts.

blender -b --disable-autoexec --python-exit-code 1 \
  esp32s3_watch_box_fit_check.blend --python doc/enclosure/review_components.py
Output is a NEW file. Unmeasured reference parts remain explicitly unverified.
"""
from pathlib import Path
import hashlib
import json
import math
import sys
import bpy
import bmesh
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from fit_audit import audit, prepared, intersects
scene = bpy.context.scene
source = Path(bpy.data.filepath)
assert source.name == 'esp32s3_watch_box_fit_check.blend', 'Load the edited fit-check file first'
assembly = bpy.data.collections['COMPONENTS — editable references, original case unchanged']

def find(prefix):
    return next(o for o in scene.objects if o.name.startswith(prefix))

def bounds(obj):
    points = [obj.matrix_world @ Vector(v) for v in obj.bound_box]
    return [[min(v[i] for v in points), max(v[i] for v in points)] for i in range(3)]

def update():
    bpy.context.view_layer.update()

def fingerprint(obj):
    data = {'matrix': [list(r) for r in obj.matrix_world],
            'modifiers': [(m.name, m.type, getattr(getattr(m, 'object', None), 'name', None),
                           getattr(m, 'operation', None)) for m in obj.modifiers]}
    if obj.type == 'MESH':
        data['vertices'] = [list(v.co) for v in obj.data.vertices]
        data['faces'] = [list(p.vertices) for p in obj.data.polygons]
    return hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest()

# Protect all existing shell, lid, port-cutter and gauge objects, not just CASE.
protected = [o for o in scene.objects if o.type == 'MESH' and o.parent is None
             and not any(c.name.startswith(('FIT WARNINGS', 'COMPONENTS', 'CLEARANCE', 'UNPLACED', 'OPTION'))
                         for c in o.users_collection)]
before = {o.name: fingerprint(o) for o in protected}
source_sha = hashlib.sha256(source.read_bytes()).hexdigest()
cap_was_present = any(o.name.startswith('Black grip cap') for o in scene.objects)

# Tape is part of the supplied 5.7 mm battery envelope, not extra thickness.
find('Battery terminal tape').location.z = 5.6
mouth = find('SD socket opening')
mouth['visual_only'] = True
mouth.dimensions.y = .02
mouth.location.y = -9.99
for obj in scene.objects:
    if obj.name.startswith('LCD rear SMD'):
        obj['visual_only'] = True
        obj['clearance_only'] = True
        obj['note'] = 'Unknown rear component positions; a whole-board allowance is not solid hardware'

# Read the real cutter. Hardcoded X=-5 was wrong for the user's current shell.
usb_tool, usb, esp = find('USB-C ('), find('ESP CAD 35'), find('ESP32-S3-Zero — official')
b = bounds(usb)
esp.location += Vector((usb_tool.location.x - sum(b[0]) / 2,
                        usb_tool.location.y + .65 - b[1][0],
                        usb_tool.location.z - sum(b[2]) / 2))
esp['placement_note'] = 'USB aligned to current cutter; mouth recessed 0.65 mm; plug access needs coupon check'
update()
chip = bounds(find('ESP CAD 36'))
heat = find('HEATSINK —')
heat.location = (sum(chip[0]) / 2, sum(chip[1]) / 2, chip[2][1] + .2)

# Flip the reader, keeping the socket facing the existing SD window.
# Height and shell shape are still references; no manufacturer CAD was supplied.
sd = find('SD MODULE —')
sd.rotation_euler.y = math.pi
sd.location = (10, -6.5, 9.95)
sd['placement_note'] = 'Socket faces -Y; flipped board clears space below; height remains unmeasured'
reserve = find('SD card reserve +15')
reserve.parent = sd
reserve.location = (0, 6.5, 1.9)
reserve.rotation_euler = (0, 0, 0)
reserve['clearance_only'] = True
reserve['note'] = '20+15 mm conservative user reserve; follows reader, not a physical card model'

# Recess the metal face inside the wall. Keep external washer/nut outside it.
enc = find('ENCODER — bare')
enc.location.y = 10.3
enc.location.z = 6.25
find('Encoder mounting washer').location.y = 18.65 - enc.location.y
find('Encoder hex mounting nut').location.y = 19.7 - enc.location.y
enc['placement_note'] = 'Reference 12.4 mm body leaves only 0.05 mm above/below in shell; actual Z unknown'

# Inspect the full, unchanged pouch beside ESP. Its collision with the encoder
# is deliberately retained and reported, rather than hidden by shrinking it.
battery = find('BATTERY —')
battery.rotation_euler.z = math.pi / 2
battery.location = (11.2, -2, .9)
battery['note'] = '28x27x5.7 unchanged; candidate beside ESP, encoder conflict remains'
tps = find('TPS61023 —')
tps.rotation_euler.z = math.pi / 2
tps.location = (-13.5, 10.5, 6.65)
tps['note'] = '18x11x5 unchanged; upper-left candidate, internal shapes approximate'
update()

# Remove obsolete warning boxes, never original cutters or user components.
warnings = bpy.data.collections.get('FIT WARNINGS — wire overlap regions, toggle off to inspect')
if warnings:
    for obj in list(warnings.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
else:
    warnings = bpy.data.collections.new('FIT WARNINGS — wire overlap regions, toggle off to inspect')
    scene.collection.children.link(warnings)

report = audit(scene, assembly, protected)
# Include both covers: a body-only check misses interference at top and bottom.
deps = bpy.context.evaluated_depsgraph_get()
for shell in protected:
    if not shell.name.startswith(('NEW FRONT COVER', 'NEW REAR COVER')):
        continue
    shape = prepared(shell, deps)
    for root in [o for o in assembly.objects if o.type == 'EMPTY']:
        hits = []
        for obj in root.children:
            if obj.type == 'MESH' and not obj.get('visual_only', False):
                hit = intersects(prepared(obj, deps), shape)
                if hit:
                    hits.append({'part': obj.name, **hit})
        if hits:
            report['case_collisions'].append({'component': root.name, 'shell': shell.name, 'intersections': hits})

# BVH surfaces can falsely flag touching/coplanar geometry. Resolve candidates
# with closed Boolean volumes; an open Boolean result remains inconclusive.
def exact_overlap(a, b):
    copies = []
    try:
        deps = bpy.context.evaluated_depsgraph_get()
        for original in (a, b):
            mesh = bpy.data.meshes.new_from_object(original.evaluated_get(deps))
            mesh.transform(original.matrix_world)
            bm = bmesh.new(); bm.from_mesh(mesh)
            bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=.00001)
            bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
            bm.to_mesh(mesh); bm.free()
            obj = bpy.data.objects.new('TEMP exact overlap', mesh)
            scene.collection.objects.link(obj); copies.append(obj)
        bpy.context.view_layer.objects.active = copies[0]
        mod = copies[0].modifiers.new('Closed intersection check', 'BOOLEAN')
        mod.operation = 'INTERSECT'; mod.solver = 'EXACT'; mod.object = copies[1]
        bpy.ops.object.modifier_apply(modifier=mod.name)
        bm = bmesh.new(); bm.from_mesh(copies[0].data)
        nonmanifold = sum(not e.is_manifold for e in bm.edges)
        volume = abs(bm.calc_volume(signed=True)); bm.free()
        return {'volume_mm3': round(volume, 6), 'nonmanifold_edges': nonmanifold,
                'classification': 'inconclusive' if nonmanifold else 'penetration' if volume > .0001 else 'no_volume'}
    finally:
        for obj in copies:
            mesh = obj.data
            bpy.data.objects.remove(obj, do_unlink=True)
            bpy.data.meshes.remove(mesh)

for row in report['component_collisions']:
    for hit in row['intersections']:
        hit['exact_check'] = exact_overlap(scene.objects[hit['a_part']], scene.objects[hit['b_part']])
body = find('CASE —')
for row in report['case_collisions']:
    shell = scene.objects.get(row.get('shell', body.name))
    for hit in row['intersections']:
        hit['exact_check'] = exact_overlap(scene.objects[hit['part']], shell)

# Retain the broad-phase evidence, but don't display a zero-volume candidate as
# a proven collision. Unknown/open intersections are visibly unresolved.
report['broad_phase_candidates'] = {k: report[k] for k in ('component_collisions', 'case_collisions')}
for key in ('component_collisions', 'case_collisions'):
    report[key] = [{**row, 'intersections': [h for h in row['intersections']
                   if h['exact_check']['classification'] != 'no_volume']} for row in report[key]]
    report[key] = [row for row in report[key] if row['intersections']]
report['component_pair_count'] = len(report['component_collisions'])
report['case_collision_group_count'] = len(report['case_collisions'])
report['collision_groups'] = sorted({row[k] for row in report['component_collisions'] for k in ('a', 'b')}
                                    | {row['component'] for row in report['case_collisions']})
for row in report['component_collisions'] + report['case_collisions']:
    for hit in row['intersections']:
        region = hit['overlap_box']
        bpy.ops.mesh.primitive_cube_add(size=1)
        obj = bpy.context.object
        obj.name = 'CHECK — ' + hit.get('a_part', hit.get('part')) + ' / ' + hit.get('b_part', row.get('shell', body.name))
        for c in list(obj.users_collection): c.objects.unlink(obj)
        warnings.objects.link(obj)
        obj.location = [sum(r) / 2 for r in region]
        obj.dimensions = [r[1] - r[0] for r in region]
        obj.display_type = 'WIRE'; obj.hide_render = True; obj.color = (1, .03, .01, 1)
        obj['evidence'] = json.dumps(hit['exact_check'])

assert before == {o.name: fingerprint(o) for o in protected}, 'Original case geometry changed'
assert cap_was_present == any(o.name.startswith('Black grip cap') for o in scene.objects), 'User deletion lost'
for root in [o for o in assembly.objects if o.type == 'EMPTY']:
    root['fit_status'] = 'COLLISION_OR_UNRESOLVED' if root.name in report['collision_groups'] else 'REFERENCE_CLEAR_NOT_CERTIFIED'
report.update(source_file=source.name, source_sha256=source_sha,
              original_objects_preserved=before, user_cap_presence_preserved=cap_was_present,
              readiness='NOT_READY_FOR_FULL_PRINT', unplaced_components=['MCP73871 — dimensions unknown'],
              method='BVH candidates, then EXACT Boolean closed-volume checks; open results inconclusive',
              limits='No certification of manufacturing tolerance, wire routing or unknown component geometry',
              placements={o.name: {'location': list(o.location), 'rotation_rad': list(o.rotation_euler)}
                          for o in assembly.objects if o.type == 'EMPTY'})
(ROOT / 'doc/enclosure/reviewed-fit.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
scene['fit_readiness'] = 'NOT_READY_FOR_FULL_PRINT — see doc/enclosure/REVIEWED_FIT.md'
# A readable starting view, without the old all-object x-ray clutter.
for obj in protected:
    if obj.name.startswith(('ENCODER (', 'USB-C (', 'microSD (', 'NEW FRONT COVER', 'NEW REAR COVER', 'PCB 4-hole')): obj.hide_set(True)
for obj in find('DISPLAY —').children:
    obj.hide_set(True)  # Inspection view only; screen geometry remains available.
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type == 'VIEW_3D':
            area.spaces.active.shading.show_xray = False
            area.spaces.active.shading.color_type = 'MATERIAL'
            area.spaces.active.overlay.show_extras = False
            area.spaces.active.region_3d.view_rotation = Vector((-65, 85, -110)).to_track_quat('-Z', 'Y')
            area.spaces.active.region_3d.view_distance = 100
            area.spaces.active.region_3d.view_location = (0, 0, 6)
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT / 'esp32s3_watch_box_reviewed.blend'))
print('REVIEW_RESULT', report['component_pair_count'], report['case_collision_group_count'], report['collision_groups'])
