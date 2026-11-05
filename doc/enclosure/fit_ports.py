"""Fit openings to the current component models, preserving the user's source.

Run in Blender background mode on esp32s3_watch_box_reviewed.blend. The new
file keeps the shell's base mesh and both covers; only its three cutters change.
Dimensions are millimetres (scene scale 0.001). SD/encoder remain references.
"""
from pathlib import Path
import hashlib
import json
import math
import struct
import sys

import bpy
import bmesh
from mathutils import Matrix, Vector, Quaternion

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'doc/enclosure/port_fit'
OUT.mkdir(exist_ok=True)
sys.path.insert(0, str(Path(__file__).resolve().parent))
from fit_audit import prepared, intersects

scene = bpy.context.scene
source = Path(bpy.data.filepath)
assert source.name == 'esp32s3_watch_box_reviewed.blend', 'Load the current reviewed file'
source_hash = hashlib.sha256(source.read_bytes()).hexdigest()

def find(prefix):
    matches = [o for o in scene.objects if o.name.startswith(prefix)]
    assert len(matches) == 1, (prefix, [o.name for o in matches])
    return matches[0]


def bounds(obj):
    points = [obj.matrix_world @ Vector(v) for v in obj.bound_box]
    return [[min(v[i] for v in points), max(v[i] for v in points)] for i in range(3)]


def fingerprint(obj, transform=True):
    record = {'vertices': [list(v.co) for v in obj.data.vertices],
              'faces': [list(f.vertices) for f in obj.data.polygons]}
    if transform:
        record['matrix'] = [list(row) for row in obj.matrix_world]
    return hashlib.sha256(json.dumps(record, sort_keys=True).encode()).hexdigest()


originals = [o for o in scene.objects if o.type == 'MESH']
before = {o.name: fingerprint(o) for o in originals}
mesh_before = {o.name: fingerprint(o, False) for o in originals}
body = find('CASE —')
rear = find('NEW REAR COVER')
esp = find('ESP32-S3-Zero — official')
heat = find('HEATSINK —')
usb = find('ESP CAD 35')
usb_tool, sd_tool, encoder_tool = [find(p) for p in ('USB-C (', 'microSD (', 'ENCODER (')]
old_openings = {o.name: bounds(o) for o in (usb_tool, sd_tool, encoder_tool)}

# Read the actual cover surface below the PCB; its rim reaches higher than its
# floor, so using the cover's bounding-box maximum would leave the board floating.
deps = bpy.context.evaluated_depsgraph_get()
floor_shape = prepared(rear, deps)
floor_samples = []
for x, y in [(-19, -14), (-4, -14), (-19, 5), (-4, 5), (-12, -5)]:
    p, _, _, _ = floor_shape['bvh'].ray_cast(Vector((x, y, 3)), Vector((0, 0, -1)), 10)
    assert p is not None, 'Rear cover must be present under the PCB'
    floor_samples.append(p.z)
assert max(floor_samples) - min(floor_samples) < .001
floor = max(floor_samples)
insulation_gap = .3
pcb = bpy.data.objects['ESP CAD 0']
old_pcb_bottom = bounds(pcb)[2][0]
footprint = bounds(pcb)

# Clip evaluated cover triangles to the PCB footprint. A few interior rays miss
# the narrow retaining lip under the PCB edge. Its highest point limits the
# drop even though the broad interior floor is lower.
def clip(poly, axis, limit, keep_above):
    if not poly: return []
    result = []
    previous = poly[-1]
    prev_inside = previous[axis] >= limit if keep_above else previous[axis] <= limit
    for current in poly:
        cur_inside = current[axis] >= limit if keep_above else current[axis] <= limit
        if cur_inside != prev_inside:
            fraction = (limit - previous[axis]) / (current[axis] - previous[axis])
            result.append(previous.lerp(current, fraction))
        if cur_inside: result.append(current)
        previous, prev_inside = current, cur_inside
    return result

support_top = floor
cover = rear.evaluated_get(deps); cover_mesh = cover.to_mesh(); cover_mesh.calc_loop_triangles()
for triangle in cover_mesh.loop_triangles:
    poly = [rear.matrix_world @ cover_mesh.vertices[i].co for i in triangle.vertices]
    for axis in (0, 1):
        poly = clip(poly, axis, footprint[axis][0], True)
        poly = clip(poly, axis, footprint[axis][1], False)
    if poly: support_top = max(support_top, max(p.z for p in poly))
cover.to_mesh_clear()
drop = support_top + insulation_gap - old_pcb_bottom
assert drop < 0, 'Expected a downward move'
esp.location.z += drop
# The heatsink is a separate parent in the user's file; preserve its thermal
# interface gap instead of leaving it at the old height.
heat.location.z += drop
bpy.context.view_layer.update()
esp['placement_note'] = 'PCB underside 0.30 mm above highest rear-cover lip under board; allow insulation here'
heat['placement_note'] = 'Moved with ESP; original chip/interface spacing preserved'


def replace_cutter(obj, width, height, depth, radius, center):
    """Closed rounded rectangle extruded along Y; preserve modifier references."""
    radius = min(radius, width / 2, height / 2)
    contour = []
    for cx, cz, angle in [(width/2-radius, height/2-radius, 0),
                           (-width/2+radius, height/2-radius, 90),
                           (-width/2+radius, -height/2+radius, 180),
                           (width/2-radius, -height/2+radius, 270)]:
        for i in range(17):
            a = math.radians(angle + i * 90 / 16)
            contour.append((cx + radius * math.cos(a), cz + radius * math.sin(a)))
    count = len(contour)
    vertices = [(x, y, z) for y in (-depth/2, depth/2) for x, z in contour]
    faces = [tuple(range(count-1, -1, -1)), tuple(range(count, count*2))]
    faces += [(i, (i+1) % count, (i+1) % count + count, i + count) for i in range(count)]
    mesh = bpy.data.meshes.new(obj.name + ' fitted mesh')
    mesh.from_pydata(vertices, [], faces)
    bm = bmesh.new(); bm.from_mesh(mesh)
    bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=.000001)
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    assert all(e.is_manifold for e in bm.edges)
    bm.to_mesh(mesh); bm.free()
    obj.data = mesh
    obj.matrix_world = Matrix.Translation(Vector(center))
    obj.display_type = 'WIRE'; obj.hide_render = True
    obj['opening_mm'] = [width, height]
    obj['clearance_each_side_mm'] = .3


# Fit the front section, not the USB's rear solder tabs. Those tabs are taller
# and do not pass through the wall; their full bounds created the oversized hole.
points = [usb.matrix_world @ v.co for v in usb.data.vertices]
front_y = min(v.y for v in points)
front = [v for v in points if v.y < front_y + .1]
xmin, xmax = min(v.x for v in front), max(v.x for v in front)
zmin, zmax = min(v.z for v in front), max(v.z for v in front)
replace_cutter(usb_tool, xmax-xmin+.6, zmax-zmin+.6, 4.6, .8,
               ((xmin+xmax)/2, usb_tool.location.y, (zmin+zmax)/2))

# This mouth was explicitly labelled visual-only in the previous review. Keep
# that uncertainty in the report; a tight-looking reference is not a measurement.
mouth = bounds(find('SD socket opening'))
replace_cutter(sd_tool, mouth[0][1]-mouth[0][0]+.6, mouth[2][1]-mouth[2][0]+.6,
               4.6, .35, ((mouth[0][0]+mouth[0][1])/2, sd_tool.location.y,
                         (mouth[2][0]+mouth[2][1])/2))
bushing = bounds(find('Encoder threaded bushing'))
replace_cutter(encoder_tool, 7.8, 7.8, 6.6, 3.9,
               ((bushing[0][0]+bushing[0][1])/2, encoder_tool.location.y,
                (bushing[2][0]+bushing[2][1])/2))
bpy.context.view_layer.update()

# Old warning boxes refer to the previous positions, so hide them rather than
# presenting stale diagnostics over the new assembly.
for collection in bpy.data.collections:
    if collection.name.startswith('FIT WARNINGS'):
        collection.hide_viewport = True; collection.hide_render = True

moved = set(o.name for root in (esp, heat) for o in root.children_recursive if o.type == 'MESH')
cutters = {o.name for o in (usb_tool, sd_tool, encoder_tool)}
for obj in originals:
    if obj.name not in cutters:
        assert fingerprint(obj, False) == mesh_before[obj.name], obj.name
    if obj.name not in moved | cutters:
        assert fingerprint(obj) == before[obj.name], obj.name


def world_copy(original, name):
    mesh = bpy.data.meshes.new_from_object(original.evaluated_get(bpy.context.evaluated_depsgraph_get()))
    mesh.transform(original.matrix_world)
    obj = bpy.data.objects.new(name, mesh); scene.collection.objects.link(obj)
    return obj


def mesh_stats(mesh):
    bm = bmesh.new(); bm.from_mesh(mesh)
    bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=.00001)
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    stats = {'nonmanifold_edges': sum(not e.is_manifold for e in bm.edges),
             'volume_mm3': abs(bm.calc_volume(signed=True))}
    bm.to_mesh(mesh); bm.free()
    return stats


def overlap(a, b):
    first, second = world_copy(a, 'TEMP intersection A'), world_copy(b, 'TEMP intersection B')
    try:
        mesh_stats(first.data); mesh_stats(second.data)
        mod = first.modifiers.new('Verify clearance', 'BOOLEAN'); mod.operation = 'INTERSECT'
        mod.solver = 'EXACT'; mod.object = second
        bpy.context.view_layer.objects.active = first
        bpy.ops.object.modifier_apply(modifier=mod.name)
        return mesh_stats(first.data)
    finally:
        for obj in (first, second):
            mesh = obj.data; bpy.data.objects.remove(obj, do_unlink=True); bpy.data.meshes.remove(mesh)


checks = {'USB to body': overlap(usb, body), 'PCB to rear cover': overlap(pcb, rear),
          'encoder bushing to body': overlap(find('Encoder threaded bushing'), body)}
for label, result in checks.items():
    assert result['nonmanifold_edges'] == 0 and result['volume_mm3'] < .0001, (label, result)


def export_stl(obj, path, flatten=False):
    # Bake evaluated booleans. Binary STL has no units: coordinates here are mm.
    if flatten:
        for v in obj.data.vertices:
            x, y, z = v.co; v.co = (x, -z, y)
    low = Vector([min(v.co[i] for v in obj.data.vertices) for i in range(3)])
    for v in obj.data.vertices: v.co -= low
    stats = mesh_stats(obj.data)
    assert stats['nonmanifold_edges'] == 0 and stats['volume_mm3'] > 0, (path, stats)
    obj.data.calc_loop_triangles()
    with path.open('wb') as f:
        f.write(('Watch fitted ports; units mm; '+path.name).encode()[:80].ljust(80, b'\0'))
        f.write(struct.pack('<I', len(obj.data.loop_triangles)))
        for tri in obj.data.loop_triangles:
            points = [obj.data.vertices[i].co for i in tri.vertices]
            normal = (points[1]-points[0]).cross(points[2]-points[0]).normalized()
            f.write(struct.pack('<12fH', *normal, *points[0], *points[1], *points[2], 0))
    stats['dimensions_mm'] = [max(v.co[i] for v in obj.data.vertices) for i in range(3)]
    stats['sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
    mesh = obj.data; bpy.data.objects.remove(obj, do_unlink=True); bpy.data.meshes.remove(mesh)
    return stats


exports = {}
exports['case.stl'] = export_stl(world_copy(body, 'TEMP case export'), OUT/'case.stl')
for original, filename in [(find('NEW FRONT COVER'), 'front-cover.stl'), (rear, 'rear-cover.stl')]:
    exports[filename] = export_stl(world_copy(original, 'TEMP cover export'), OUT/filename)

# A small coupon lets the user check the tighter apertures and plug access
# without spending a full case print. Retain just this side's physical wall.
coupon = world_copy(body, 'TEMP port coupon')
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, -18.4, 6.25))
box = bpy.context.object; box.dimensions = (43, 3.5, 12.5)
bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
mod = coupon.modifiers.new('Extract USB SD wall', 'BOOLEAN'); mod.operation = 'INTERSECT'; mod.solver = 'EXACT'; mod.object = box
bpy.context.view_layer.objects.active = coupon; bpy.ops.object.modifier_apply(modifier=mod.name)
bpy.data.objects.remove(box, do_unlink=True)
exports['usb-sd-coupon.stl'] = export_stl(coupon, OUT/'usb-sd-coupon.stl', True)

# Show an understandable whole assembly on first open, instead of the user's
# previous extreme close-up. Covers remain available and keep their geometry.
bpy.ops.object.select_all(action='DESELECT'); body.hide_set(False); body.select_set(True)
bpy.context.view_layer.objects.active = body
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type == 'VIEW_3D':
            area.spaces.active.overlay.show_extras = False
            area.spaces.active.shading.show_xray = False
            region = area.spaces.active.region_3d
            region.view_location = Vector((0, 0, 6)); region.view_distance = 85
            region.view_rotation = Quaternion((.88, .46, .06, .09)).normalized()

report = {'source_file': source.name, 'source_sha256': source_hash,
          'output_file': 'esp32s3_watch_box_ports_fitted.blend',
          'pcb_old_bottom_z_mm': old_pcb_bottom, 'rear_floor_z_mm': floor,
          'rear_support_top_z_mm': support_top,
          'esp_delta_z_mm': drop, 'pcb_insulation_gap_mm': insulation_gap,
          'openings_before_world_bounds_mm': old_openings,
          'openings_after_world_bounds_mm': {o.name: bounds(o) for o in (usb_tool, sd_tool, encoder_tool)},
          'checks': checks, 'exports': exports,
          'limits': ['SD mouth and encoder thread are unmeasured reference models.',
                     'Plug overmould and card removal access require physical coupon testing.',
                     'Other component positions are preserved; known battery/display/encoder fit issues are not resolved.',
                     '0.30 mm under PCB is reserved for insulation, not a new printed mounting post.']}
(OUT/'dimensions.json').write_text(json.dumps(report, indent=2)+'\n')
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/report['output_file']))
assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash, 'Source changed'
print('FITTED PORTS: source preserved; ESP delta Z', drop, '; checks', checks)
