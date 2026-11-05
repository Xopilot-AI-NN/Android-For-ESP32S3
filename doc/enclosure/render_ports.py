"""Render the saved fitted assembly without changing its blend file."""
from pathlib import Path
import math
import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[2]
scene = bpy.context.scene
assert Path(bpy.data.filepath).name == 'esp32s3_watch_box_ports_fitted.blend'
scene.render.engine = 'BLENDER_WORKBENCH'
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = 'PNG'
scene.render.film_transparent = False
scene.display.shading.light = 'STUDIO'
scene.display.shading.studiolight_rotate_z = math.radians(35)
scene.display.shading.color_type = 'MATERIAL'
scene.display.shading.show_shadows = True
scene.display.shading.show_cavity = True
scene.display.shading.cavity_type = 'BOTH'
scene.display.shading.show_specular_highlight = True
scene.display.shading.background_type = 'WORLD'
scene.world.color = (.055, .065, .08)
scene.view_settings.view_transform = 'Standard'

for o in scene.objects:
    if o.type == 'MESH' and (o.display_type == 'WIRE' or o.hide_get()):
        o.hide_render = True
    if o.name.startswith(('CHECK', 'TEMP', 'LCD rear SMD', 'SD socket opening')):
        o.hide_render = True

camera_data = bpy.data.cameras.new('Inspection camera')
camera = bpy.data.objects.new('Inspection camera', camera_data)
scene.collection.objects.link(camera); scene.camera = camera; camera_data.type = 'ORTHO'

def shot(name, position, target, scale, width, height):
    camera.location = position
    camera.rotation_euler = (Vector(target) - camera.location).to_track_quat('-Z', 'Y').to_euler()
    camera_data.ortho_scale = scale
    camera_data.clip_start = .01; camera_data.clip_end = 1000
    scene.render.resolution_x = width; scene.render.resolution_y = height
    scene.render.filepath = str(ROOT/'doc/enclosure/port_fit'/name)
    bpy.ops.render.render(write_still=True)

shot('ports.png', (0, -90, 6), (0, 0, 6), 56, 1400, 460)
# Hide the screen and front lid only for the inspection view. No geometry or
# saved visibility is changed: this script never saves the blend file.
for o in scene.objects:
    if o.name.startswith(('NEW FRONT COVER', 'FRONT BEZEL')) or (o.parent and o.parent.name.startswith('DISPLAY —')):
        o.hide_render = True
shot('inside.png', (44, -65, 74), (0, 0, 4), 76, 1200, 900)
print('RENDERED: port elevation and lid-off inspection')
