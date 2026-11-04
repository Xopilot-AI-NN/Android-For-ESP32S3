"""Build a fit-audit model with source-backed parts and explicit unresolved dimensions.
Run: blender -b --disable-autoexec --python-exit-code 1 esp32s3_watch_box.blend --python doc/enclosure/add_components_only.py
"""
from pathlib import Path
import hashlib
import json
import math
import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[2]
scene = bpy.context.scene
original = list(scene.objects)

def geometry_record(obj):
    record={'name':obj.name,'matrix':[list(row) for row in obj.matrix_world],
            'hidden':obj.hide_get(),'hide_render':obj.hide_render,
            'collections':sorted(c.name for c in obj.users_collection),
            'modifiers':[(m.name,m.type,getattr(getattr(m,'object',None),'name',None)) for m in obj.modifiers]}
    if obj.type=='MESH':
        record['vertices']=[list(v.co) for v in obj.data.vertices]
        record['faces']=[list(f.vertices) for f in obj.data.polygons]
        record['edges']=[list(e.vertices) for e in obj.data.edges]
    return record

before=[geometry_record(obj) for obj in original]
source_digest=hashlib.sha256(json.dumps(before,sort_keys=True).encode()).hexdigest()

def collection(name):
    c=bpy.data.collections.new(name);scene.collection.children.link(c);return c

assembly=collection('COMPONENTS — editable references, original case unchanged')
optional=collection('OPTION — old TP4056, not simultaneous with MCP73871')
clearances=collection('CLEARANCE — card reserve, approximate')
unplaced=collection('UNPLACED — do not count as fitted components')

def material(name,color):
    m=bpy.data.materials.new(name);m.diffuse_color=(*color,1);m.use_nodes=True
    m.node_tree.nodes.clear()
    shader=m.node_tree.nodes.new('ShaderNodeBsdfPrincipled')
    output=m.node_tree.nodes.new('ShaderNodeOutputMaterial')
    m.node_tree.links.new(shader.outputs['BSDF'],output.inputs['Surface'])
    shader.inputs['Base Color'].default_value=(*color,1)
    shader.inputs['Roughness'].default_value=.55
    return m

pcb=material('REF PCB blue',(.045,.21,.38))
green=material('REF PCB green',(.08,.34,.20))
metal=material('REF connector metal',(.50,.57,.63))
black=material('REF IC and plastic',(.035,.045,.055))
battery_mat=material('REF battery pouch',(.58,.65,.68))
red=material('REF charger — dimensions assumed',(.70,.18,.16))
glass=material('REF display glass',(.018,.028,.033))
contacts=material('REF contacts',(.65,.42,.09))

def move_to(obj,target):
    for c in list(obj.users_collection):c.objects.unlink(obj)
    target.objects.link(obj)

records=[]
def group(name,location,size,known_xyz,note='',target=assembly):
    obj=bpy.data.objects.new(name,None);target.objects.link(obj);obj.location=location
    obj.empty_display_type='PLAIN_AXES';obj.empty_display_size=2
    obj['nominal_size_mm']=size;obj['known_xyz']=known_xyz;obj['note']=note
    records.append({'name':name,'size_mm':size,'known_xyz':known_xyz,'note':note,'position_mm':list(location)})
    return obj

def box(parent,name,size,offset,mat,target=assembly):
    bpy.ops.mesh.primitive_cube_add(size=1)
    obj=bpy.context.object;obj.name=name;move_to(obj,target)
    obj.parent=parent;obj.location=offset;obj.dimensions=size
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    obj.data.materials.append(mat)
    return obj

def cylinder(parent,name,radius,depth,offset,mat,axis='Z',target=assembly):
    bpy.ops.mesh.primitive_cylinder_add(vertices=32,radius=radius,depth=depth)
    obj=bpy.context.object;obj.name=name;move_to(obj,target)
    obj.parent=parent;obj.location=offset
    if axis=='Y':obj.rotation_euler.x=math.pi/2
    obj.data.materials.append(mat)
    return obj

# Component origins correspond approximately to the existing port cutters.
# All dimensions are in the original file's millimetre coordinates.
# The 48×31 drawing matches the supplied nominal size. Treat its revision as
# a candidate until the user's board is identified; do not silently assert identity.
display=group('DISPLAY — LCDWIKI MSP1691 reference, revision unconfirmed',(0,0,12.5),[48,31,4.56],False,'Candidate drawing: PCB 1.60, adhesive 0.30, LCD 1.56, rear SMD <=1.10; no headers')
display['source']='https://www.lcdwiki.com/res/MSP1691/MSP1691_Size.pdf'
display['mechanical_source']='manufacturer_drawing_candidate'
display['identity_confirmed']=False
plate=box(display,'LCD PCB — 1.60 mm',(48,31,1.6),(0,0,.8),pcb)
# Four mounting holes: pitch 42.99×26.00, nominal drill inferred as 2.0 mm
# from the visual drawing, not accepted as a verified drill specification.
for x in [-21.5,21.49]:
    for y in [-13,13]:
        tool=cylinder(display,'TEMP hole',1,4,(x,y,.8),black)
        bpy.context.view_layer.update();bpy.context.view_layer.objects.active=plate
        modifier=plate.modifiers.new('Mounting hole — drill diameter unconfirmed','BOOLEAN');modifier.operation='DIFFERENCE';modifier.solver='EXACT';modifier.object=tool
        bpy.ops.object.modifier_apply(modifier=modifier.name);bpy.data.objects.remove(tool,do_unlink=True)
box(display,'LCD adhesive — 0.30 mm',(37.43,30.07,.3),(.005,-.005,1.75),black)
box(display,'LCD module — 37.43×30.07×1.56',(37.43,30.07,1.56),(.005,-.005,2.68),glass)
active_area=box(display,'LCD active area — 32.634×27.972', (32.634,27.972,.02),(-1.293,.001,3.47),green)
active_area['visual_only']=True
for y in [(-15.5+6.61+i*2.54) for i in range(8)]:
    cylinder(display,'LCD solder pad',.8,.08,(-22.5,y,1.64),contacts)
# Rear components and soldered wire bends need measurement, so show their whole
# allowed layer as a wireframe clearance rather than inventing component positions.
rear=box(display,'LCD rear SMD <=1.10 — clearance only',(48,31,1.1),(0,0,-.55),contacts,clearances)
rear.display_type='WIRE';rear.hide_render=True

cad=json.loads((ROOT/'doc/enclosure/references/esp32-s3-zero-mesh.json').read_text())
usb=cad['solids'][35]['bounds']
usb_center_z=(usb[4]+usb[5])/2
usb_front=usb[3]
z_offset=6.4-usb_center_z
y_offset=-18.1+usb_front
esp=group('ESP32-S3-Zero — official Waveshare v2 CAD',(-5,y_offset,z_offset),[18.017343,24.742648,4.912132],False,'Official STEP, unscaled. Confirm board revision; hand-soldered wires not included')
esp['mechanical_source']='manufacturer_step';esp['identity_confirmed']=False
esp['source']='https://files.waveshare.com/wiki/ESP32-S3-Zero/ESP32-S3-Zero_v2-138eb3753cf14afa3252058c380ca416.stp'
# STEP USB mouth points along +Y. Rotate into the existing -Y opening.
for solid in cad['solids']:
    mesh=bpy.data.meshes.new('ESP official solid '+str(solid['index']))
    vertices=[(9-x,-y,z) for x,y,z in solid['vertices']]
    mesh.from_pydata(vertices,[],solid['faces']);mesh.update()
    obj=bpy.data.objects.new('ESP CAD '+str(solid['index']),mesh);assembly.objects.link(obj);obj.parent=esp
    obj['cad_solid_index']=solid['index']
    obj.data.materials.append(pcb if solid['index']==0 else metal if solid['index']==35 else black)
chip=cad['solids'][36]['bounds']
chip_center=(-5+9-(chip[0]+chip[1])/2,y_offset-(chip[2]+chip[3])/2)
chip_top=z_offset+chip[5]

sd=group('SD MODULE — 20×18, height assumed',(10,-7.1,5.0),[18,20,3.85],False,'Port alignment and inserted-card projection remain approximate')
box(sd,'SD PCB',(18,20,1),(0,0,.5),pcb)
box(sd,'SD socket metal',(15.2,18,2.4),(0,-1,2.65),metal)
box(sd,'SD socket opening',(13,.15,1.2),(0,-10.05,2.65),black)
reserve=group('SD card reserve +15 mm',(10,-.6,6.9),[18,35,1],False,'Conservative 20+15 envelope requested by user; not an actual card dimension',clearances)
obj=box(reserve,'SD conservative insertion envelope',(18,35,1),(0,0,0),contacts,clearances)
obj.display_type='WIRE';obj.hide_render=True

encoder=group('ENCODER — board 15×17, height assumed',(8.8,8.6,2),[15,17,8.2],False,'Shaft follows existing cutter axis; board/body/shaft dimensions are approximate')
box(encoder,'Encoder PCB',(15,17,1),(0,0,.5),pcb)
box(encoder,'Encoder metal body',(12,11,7),(0,2,4.5),metal)
cylinder(encoder,'Encoder shaft',3,8,(0,10.5,4),metal,'Y')

battery=group('BATTERY — selected 28×27×5.7',(-9,1.5,.9),[28,27,5.7],True,'Placed for inspection; overlaps are not resolved; wire exit and protection unknown')
box(battery,'Battery pouch',(28,27,5.7),(0,0,2.85),battery_mat)
box(battery,'Battery terminal tape',(10,2,.2),(0,12.5,5.8),black)

tps=group('TPS61023 — 18×11×5',(17,-7,.8),[11,18,5],True,'Rotated XY; external envelope known, visual internals approximate')
box(tps,'TPS PCB',(11,18,1),(0,0,.5),black)
box(tps,'TPS inductor',(7,7,4),(0,1,3),black)
box(tps,'TPS controller',(3,3,1),(0,-5,1.5),black)
cylinder(tps,'TPS input capacitor — visual approximation',2.3,3,(0,6.1,2.5),metal)
for y in [-7,7]:
    for x in [-3.5,3.5]:box(tps,'TPS solder pad',(2,2,.08),(x,y,1.04),contacts)

heatsink=group('HEATSINK — 15×10×5',(chip_center[0],chip_center[1],chip_top+.2),[15,10,5],True,'Aligned to official CAD chip; assumed thermal pad 0.20 mm; actual module identity unconfirmed')
box(heatsink,'Heatsink base',(15,10,1),(0,0,.5),metal)
for x in [-6.6,-4.4,-2.2,0,2.2,4.4,6.6]:box(heatsink,'Heatsink fin',(1.0,10,4),(x,0,3),metal)

# Do not pretend that an unmeasured board fits. Keep the old visual proxy outside
# the case, excluded from the fitted assembly, and label every dimension unknown.
charger=group('MCP73871 — NOT PLACED / dimensions unknown',(60,0,0),[32,30,3],False,'Proxy ONLY; ALL dimensions unknown; capacitor not included; Ozon 1964855217',unplaced)
charger['nominal_size_mm']='UNKNOWN — old proxy size is not a measurement'
records[-1]['size_mm']=None
records[-1]['visual_proxy_size_mm']=[32,30,3]
charger['source']='https://ozon.by/product/1964855217/'
charger['fit_status']='UNPLACED_UNKNOWN_DIMENSIONS'
obj=box(charger,'MCP73871 old visual proxy — not a fit model',(32,30,1),(0,0,.5),red,unplaced)
obj.display_type='WIRE'
obj=box(charger,'MCP IC visual proxy',(4,4,1),(0,0,1.5),red,unplaced);obj.display_type='WIRE'

old=group('TP4056 — existing alternative 29×18×5',(45,0,0),[29,18,5],True,'Alternative only; do not treat as a second parallel charger',optional)
box(old,'TP4056 PCB',(29,18,1),(0,0,.5),pcb,optional)
box(old,'TP4056 IC',(5,4,1.5),(0,0,1.75),black,optional)
box(old,'TP4056 USB connector',(8.8,6,4),(-10,0,3),metal,optional)
optional.hide_viewport=True;optional.hide_render=True

# Run the geometric audit before marking anything ready for printing.
import sys
sys.path.insert(0,str(ROOT/'doc/enclosure'))
from fit_audit import audit
bpy.context.view_layer.update()
fit_report=audit(scene,assembly,original)
fit_report['unplaced_components']=[charger.name]
fit_report['readiness']='NOT_READY_FOR_PRINT'
# Wire boxes show only the intersecting space, not invented replacement geometry.
conflicts=collection('FIT WARNINGS — wire overlap regions, toggle off to inspect')
for row in fit_report['component_collisions']:
    boxes=[hit['overlap_box'] for hit in row['intersections']]
    region=[(min(b[i][0] for b in boxes),max(b[i][1] for b in boxes)) for i in range(3)]
    bpy.ops.mesh.primitive_cube_add(size=1)
    obj=bpy.context.object;move_to(obj,conflicts)
    obj.name='COLLISION — '+row['a'].split(' — ')[0]+' / '+row['b'].split(' — ')[0]
    obj.location=[(v[0]+v[1])/2 for v in region];obj.dimensions=[v[1]-v[0] for v in region]
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    obj.display_type='WIRE';obj.hide_render=True;obj.color=(1,.05,.02,1);obj.data.materials.append(red)
    obj['note']='Envelope of reported intersecting mesh regions; inspect fit-check.json for assumptions'
(ROOT/'doc/enclosure/fit-check.json').write_text(json.dumps(fit_report,ensure_ascii=False,indent=2)+'\n')
for root in [o for o in assembly.objects if o.type=='EMPTY']:
    root['fit_status']='COLLISION' if root.name in fit_report['collision_groups'] else 'UNVERIFIED'
# Verify every original object's mesh, placement, visibility, collection and cutters.
# Newly added parts are allowed to collide; resizing the original shell is forbidden.
after=[geometry_record(obj) for obj in original]
assert before==after,'An original object was changed'
body=next(o for o in original if o.name.startswith('CASE —'))
report={'source':'esp32s3_watch_box.blend','original_objects_verified_unchanged':len(original),
        'source_geometry_sha256':source_digest,'case_dimensions_mm':list(body.dimensions),
        'components':records,'fit_verified':False,'warning':'Placement is an editable draft. Parts may overlap each other or the original shell. No case resizing or port changes were made.'}
(ROOT/'doc/enclosure/fit-check-components-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
text=bpy.data.texts.new('READ ME — original case with components')
text.write('''The original case, covers, cutters and fit gauges are unchanged.
New COMPONENTS collection contains editable visual references, grouped by board.
Move a parent empty to move the whole component; expand it for individual parts.
Use Alt+Z X-ray or hide the original CASE to inspect the contents.
Placement is preliminary and includes unresolved overlaps. This is not proof of fit.
Known XYZ: battery 28×27×5.7, TPS61023 18×11×5, heatsink 15×10×5, TP4056 29×18×5.
ESP replaced with official Waveshare STEP (confirm actual revision).
LCD replaced with LCDWIKI drawing reference (confirm actual revision).
SD and encoder heights/offsets remain assumed. MCP73871 is parked outside the case.
The external charger capacitor has not been modeled because its dimensions are unknown.
The old TP4056 alternative is hidden in its own collection, outside the case.
The SD +15 mm reserve is a wireframe envelope, not a measured card model.
No 12 mm or 30 mm target has been applied to the original case.
''')
# Selection/view settings do not change any original geometry.
bpy.ops.object.select_all(action='DESELECT')
esp.select_set(True);bpy.context.view_layer.objects.active=esp
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type=='VIEW_3D':
            area.spaces.active.shading.color_type='MATERIAL'
            area.spaces.active.shading.show_xray=True
            area.spaces.active.shading.xray_alpha=.65
            area.spaces.active.overlay.show_relationship_lines=False
            area.spaces.active.overlay.show_extras=False
            area.spaces.active.region_3d.view_location=(0,0,6)
            area.spaces.active.region_3d.view_distance=92
            area.spaces.active.region_3d.view_rotation=Vector((70,-100,80)).to_track_quat('Z','Y')
bpy.context.view_layer.update()
assert before==[geometry_record(obj) for obj in original]
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'esp32s3_watch_box_fit_check.blend'))
print('ADDED_COMPONENTS='+json.dumps({'groups':len(records),'original_objects_unchanged':len(original),'file':str(ROOT/'esp32s3_watch_box_fit_check.blend')}))
