"""Run with Blender --background --disable-autoexec ORIGINAL.blend --python this_file.

Produce a separate study file. Unknown board heights and charger dimensions are
explicit envelopes, not measured parts. The original blend is never saved over.
"""
from pathlib import Path
import json
import math
import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'doc/enclosure'
BODY_TOP = 30.0  # Adjustable study depth; not an approved manufacturing dimension.
ORIGINAL_TOP = 12.5
FIXED_BOTTOM = 3.4  # Keep the lower strap geometry at its original height.
scene = bpy.context.scene
original_objects = list(scene.objects)
reference = bpy.data.collections.new('00 ORIGINAL — reference, hidden')
scene.collection.children.link(reference)
for obj in original_objects:
    for collection in list(obj.users_collection):
        collection.objects.unlink(obj)
    reference.objects.link(obj)
reference.hide_viewport = True
reference.hide_render = True

def collection(name):
    value = bpy.data.collections.new(name)
    scene.collection.children.link(value)
    return value

shell = collection('01 CASE — depth study, ports provisional')
parts = collection('02 COMPONENT ENVELOPES — toggle visibility to inspect')
cutters = collection('03 OPENINGS — editable, positions need measurement')
notes = collection('04 DIMENSIONS — measured versus assumed')
studio = collection('05 PREVIEW — camera and lighting')

def material(name, color, alpha=1):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*color, alpha)
    mat.use_nodes = True
    mat.node_tree.nodes.clear()
    shader = mat.node_tree.nodes.new('ShaderNodeBsdfPrincipled')
    output = mat.node_tree.nodes.new('ShaderNodeOutputMaterial')
    mat.node_tree.links.new(shader.outputs['BSDF'], output.inputs['Surface'])
    shader.inputs['Base Color'].default_value = (*color, 1)
    shader.inputs['Roughness'].default_value = 0.65
    return mat

case_mat = material('Case — graphite', (0.12, 0.16, 0.20))
measured_mat = material('Known dimensions — green', (0.20, 0.65, 0.45))
unknown_mat = material('Unknown height — amber', (0.92, 0.57, 0.17))
budget_mat = material('Charger footprint UNCONFIRMED — red', (0.85, 0.23, 0.24))
metal_mat = material('Heatsink — metal envelope', (0.55, 0.65, 0.75))

def clone(source, name, destination):
    obj = source.copy()
    obj.data = source.data.copy()
    obj.name = name
    destination.objects.link(obj)
    obj.hide_set(False)
    obj.hide_render = False
    return obj

def ring_mesh(name, width, height, radius, wall, bottom, top):
    # Four corresponding loops produce a closed, constant-thickness rounded ring.
    def contour(w,h,r):
        points=[]
        for cx,cy,angle in [(w/2-r,h/2-r,0),(-w/2+r,h/2-r,90),(-w/2+r,-h/2+r,180),(w/2-r,-h/2+r,270)]:
            for step in range(13):
                a=math.radians(angle+step*90/12)
                points.append((cx+r*math.cos(a),cy+r*math.sin(a)))
        return points
    outer=contour(width,height,radius)
    inner=contour(width-2*wall,height-2*wall,max(.3,radius-wall))
    n=len(outer)
    vertices=[(x,y,z) for z in [bottom,top] for loop in [outer,inner] for x,y in loop]
    faces=[]
    for i in range(n):
        j=(i+1)%n
        faces.extend([(i,j,2*n+j,2*n+i),(n+j,n+i,3*n+i,3*n+j),
                      (j,i,n+i,n+j),(2*n+i,2*n+j,3*n+j,3*n+i)])
    mesh=bpy.data.meshes.new(name);mesh.from_pydata(vertices,[],faces);mesh.update()
    import bmesh
    bm=bmesh.new();bm.from_mesh(mesh);bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(mesh);bm.free()
    obj=bpy.data.objects.new(name,mesh);shell.objects.link(obj)
    return obj

body_source = next(o for o in original_objects if o.name.startswith('CASE —'))
# User requested no internal columns. Rebuild a plain perimeter wall rather than
# stretching the old integrated corner supports into long pillars.
body=ring_mesh('CASE v02 — perimeter walls, NO COLUMNS',52.8,36.8,4.0,1.6,0,BODY_TOP)
body.data.materials.append(case_mat)
body['fit_status']='DRAFT: 48×31 display reference; no internal posts; ports provisional'
body['body_top_mm']=BODY_TOP
body['wall_mm']=1.6
body['interior_envelope_xy_mm']=[49.6,33.6]
# Retain only the original strap tabs, with a small overlap into the new walls.
for sign in [-1,1]:
    tab=clone(body_source,'Strap anchor — original profile '+str(sign),shell)
    tab.modifiers.clear()
    bpy.ops.mesh.primitive_cube_add(size=1,location=(sign*32.85,0,6))
    clip=bpy.context.object;clip.dimensions=(14.3,60,30)
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    cut=tab.modifiers.new('Keep strap area only','BOOLEAN');cut.operation='INTERSECT';cut.solver='EXACT';cut.object=clip
    bpy.context.view_layer.objects.active=tab
    bpy.ops.object.modifier_apply(modifier=cut.name)
    bpy.data.objects.remove(clip,do_unlink=True)
    tab.data.materials.clear();tab.data.materials.append(case_mat)
    join=body.modifiers.new('Join strap anchor '+str(sign),'BOOLEAN');join.operation='UNION';join.solver='EXACT';join.object=tab
    bpy.context.view_layer.objects.active=body
    bpy.ops.object.modifier_apply(modifier=join.name)
    bpy.data.objects.remove(tab,do_unlink=True)
# Continuous lip supports the display around its perimeter; no corner columns.
ledge=ring_mesh('Display perimeter seat',52.8,36.8,4,3.2,26.2,27)
join=body.modifiers.new('Continuous display seat','BOOLEAN');join.operation='UNION';join.solver='EXACT';join.object=ledge
bpy.context.view_layer.objects.active=body;bpy.ops.object.modifier_apply(modifier=join.name)
bpy.data.objects.remove(ledge,do_unlink=True)
body['display_seat_z_mm']=27
body['display_seat_opening_mm']=[46.4,30.4]
# Keep the original movable tools. Their sizes are source references only.
for source_modifier in body_source.modifiers:
    if source_modifier.type=='BOOLEAN':
        m=body.modifiers.new(source_modifier.name,'BOOLEAN');m.operation='DIFFERENCE';m.solver='EXACT';m.object=source_modifier.object
for modifier in body.modifiers:
    if modifier.type != 'BOOLEAN' or not modifier.object:
        continue
    source = modifier.object
    tool = clone(source, source.name+' — v02 provisional', cutters)
    tool.display_type = 'WIRE'
    tool.hide_render = True
    if source.name.startswith('ENCODER'):
        tool.location.z = 12
    elif source.name.startswith('USB-C'):
        tool.location.z = 10
    else:
        tool.location.z = 19.5
    tool['fit_status'] = 'Opening size inherited; exact board/connector coordinates required'
    modifier.object = tool

front_source = next(o for o in original_objects if o.name.startswith('NEW FRONT COVER'))
rear_source = next(o for o in original_objects if o.name.startswith('NEW REAR COVER'))
front = clone(front_source, 'FRONT v02 — original aperture and four-hole pattern', shell)
front.location.z += BODY_TOP-ORIGINAL_TOP
rear = clone(rear_source, 'REAR v02 — original brass spacer seats', shell)
for obj in (front, rear):
    obj.data.materials.clear()
    obj.data.materials.append(case_mat)
    obj['fit_status'] = 'Original profile; hole pitch and inserts need physical verification'
# Hidden by default so the contents of the new working model can be inspected.
front.hide_set(True)
front.hide_render = True
rear.hide_set(True)
rear.hide_render = True

records = []
def envelope(name, size, center, measured=True, note='', mat=None):
    bpy.ops.mesh.primitive_cube_add(size=1, location=center)
    obj = bpy.context.object
    obj.name = name
    obj.dimensions = size
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    for c in list(obj.users_collection):
        c.objects.unlink(obj)
    parts.objects.link(obj)
    obj.data.materials.append(mat or (measured_mat if measured else unknown_mat))
    obj['dimensions_mm'] = list(size)
    obj['measurement_status'] = 'User measured XYZ' if measured else 'XY measured; Z assumed for layout'
    obj['note'] = note
    records.append({'name': name, 'size': list(size), 'center': list(center), 'measured_xyz': measured, 'note': note})
    return obj

envelope('BATTERY — 28×27×5.7 mm', (28,27,5.7), (0,0,4.05), note='Capacity, chemistry, protection and cable exit still unknown')
envelope('ESP + USB — 26×18; Z budget 4 mm', (26,18,4), (-10,6,10), False, 'Board rotation and USB location need actual coordinates')
envelope('ENCODER — 15×17; Z budget 8 mm', (15,17,8), (15,0,12), False, 'Shaft/knob protrusion unknown')
envelope('TPS61023 — 18×11×5 mm', (18,11,5), (-11,-10,10.5), note='Pad and wire clearance not included')
envelope('HEATSINK — 15×10×5 mm', (15,10,5), (-15,10,15.5), note='Must align with actual chip, not merely PCB', mat=metal_mat)
envelope('SD + card reserve — 35×18; Z budget 3 mm', (35,18,3), (6,-6,19.5), False, '20 mm board plus conservative 15 mm reserve; orientation and access opening unresolved')
charger = envelope('POWER PATH — 32×30×3 PLACEHOLDER, NOT measured', (32,30,3), (0,0,24.5), False, 'MCP73871 board + external capacitor not yet dimensioned', mat=budget_mat)
charger['measurement_status'] = 'ALL dimensions are assumed; not an order specification'
envelope('DISPLAY PCB — 48×31; Z budget 3 mm', (48,31,3), (0,0,28.5), False, 'Active area, glass, holes, flex and component height unknown')

# Human-readable notes travel inside the blend as well as in the external report.
text = bpy.data.texts.new('READ ME — NOT READY FOR PRINT')
text.write('''V02 is a mechanical layout study, not a verified printable assembly.
Green: measured XYZ. Amber: measured XY, assumed height. Red: unknown power module.
Body top: 30 mm, with inherited front/rear profiles. This is a depth budget only.
Original case and all original parts are retained in hidden collection 00.
New case has perimeter walls and a display lip, with no internal columns.
Front and rear are initially hidden. Toggle components to inspect the shell.
Port cutters remain editable but do NOT establish connector alignment.
No battery capacity, charge current or protection has been assumed.
MCP73871 module capacitor is NOT included in the charger box.
Mounting columns, wire routing, thermal contact, and print clearances need verification.
''')
scene.unit_settings.system = 'METRIC'
scene.unit_settings.scale_length = 0.001
scene.unit_settings.length_unit = 'MILLIMETERS'
scene['fit_status'] = 'LAYOUT STUDY — UNKNOWN DIMENSIONS, NOT READY FOR PRINT'
scene['body_top_mm'] = BODY_TOP
scene['internal_columns'] = False
scene['selected_battery_mm'] = [28,27,5.7]

# Count intersections between envelopes. The shell is intentionally excluded:
# validating clearance against pillars and apertures requires the missing dimensions.
def bounds(obj):
    vertices = [obj.matrix_world @ Vector(p) for p in obj.bound_box]
    return [(min(v[i] for v in vertices), max(v[i] for v in vertices)) for i in range(3)]
bpy.context.view_layer.update()
collisions = []
objects = list(parts.objects)
for i,a in enumerate(objects):
    for b in objects[i+1:]:
        overlaps = [min(x[1],y[1])-max(x[0],y[0]) for x,y in zip(bounds(a),bounds(b))]
        if all(d > 0.05 for d in overlaps):
            collisions.append({'a':a.name,'b':b.name,'overlap_mm':overlaps})
report = {'status':'DRAFT_NOT_FOR_PRINT','body_top_mm':BODY_TOP,'components':records,
          'envelope_intersections':collisions,'internal_columns':False,'display_reference_mm':[48,31],'shell_clearance_verified':False,
          'ports_aligned':False,'charger_capacitor_included':False}
(OUT/'layout-v02-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')

# A useful initial viewport: all measured/assumed parts in place, wireframe shell.
body.display_type = 'WIRE'
body.show_wire = True
bpy.ops.object.select_all(action='DESELECT')
body.select_set(True)
bpy.context.view_layer.objects.active = body
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type=='VIEW_3D':
            space=area.spaces.active
            space.shading.type='MATERIAL'
            space.clip_end=10000
            space.region_3d.view_distance=110
            space.region_3d.view_location=(0,0,14)
            space.region_3d.view_rotation=Vector((75,-100,85)).to_track_quat('Z','Y')

# Render the contents with a wireframe overlay of the enlarged body, avoiding an
# opaque shell that would hide the very fit assumptions this file documents.
body.hide_render=True
# Only boundary contours in the preview; dense triangulation hides the layout.
curve=bpy.data.curves.new('Preview perimeter outlines','CURVE');curve.dimensions='3D';curve.bevel_depth=.11;curve.bevel_resolution=1
for z in [0,BODY_TOP]:
    points=[]
    for cx,cy,angle in [(22.4,14.4,0),(-22.4,14.4,90),(-22.4,-14.4,180),(22.4,-14.4,270)]:
        for step in range(13):
            a=math.radians(angle+step*90/12);points.append((cx+4*math.cos(a),cy+4*math.sin(a),z))
    spline=curve.splines.new('POLY');spline.points.add(len(points)-1);spline.use_cyclic_u=True
    for p,co in zip(spline.points,points):p.co=(*co,1)
wire=bpy.data.objects.new('PREVIEW — case boundary only, not columns',curve);studio.objects.link(wire)
curve.materials.append(metal_mat);wire.hide_set(True)  # Render only; keep viewport uncluttered.
for obj in parts.objects:
    bevel=obj.modifiers.new('Preview edges only','BEVEL');bevel.width=0.25;bevel.segments=2

bpy.ops.object.camera_add(location=(86,-112,100))
camera=bpy.context.object;camera.name='Layout preview camera'
for c in list(camera.users_collection):c.objects.unlink(camera)
studio.objects.link(camera)
camera.rotation_euler=(Vector((0,0,14))-camera.location).to_track_quat('-Z','Y').to_euler()
camera.data.type='ORTHO';camera.data.ortho_scale=91
scene.camera=camera
for location,energy,size in [((30,-50,100),180000,65),((-60,20,65),110000,55)]:
    bpy.ops.object.light_add(type='AREA',location=location)
    light=bpy.context.object
    for c in list(light.users_collection):c.objects.unlink(light)
    studio.objects.link(light)
    light.data.energy=energy;light.data.shape='DISK';light.data.size=size
    light.rotation_euler=(Vector((0,0,12))-light.location).to_track_quat('-Z','Y').to_euler()
scene.world.color=(0.14,0.14,0.14)
scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=24
scene.render.resolution_x=1100;scene.render.resolution_y=950;scene.render.resolution_percentage=100
scene.render.image_settings.file_format='PNG'
scene.render.filepath=str(OUT/'layout-v02-preview.png')
# Restore the editable body selection after creating camera/lights.
bpy.ops.object.select_all(action='DESELECT');body.select_set(True);bpy.context.view_layer.objects.active=body
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'esp32s3_watch_box_v02_layout.blend'))
bpy.ops.render.render(write_still=True)
print('LAYOUT_RESULT='+json.dumps({'file':str(OUT/'esp32s3_watch_box_v02_layout.blend'),'intersections':len(collisions)}))
