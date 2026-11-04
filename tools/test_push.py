import importlib.util
from pathlib import Path
import tempfile
import unittest
from PIL import Image
spec = importlib.util.spec_from_file_location('watch_push', Path(__file__).parents[1] / 'push.py')
push = importlib.util.module_from_spec(spec)
spec.loader.exec_module(push)


class PushTests(unittest.TestCase):
    def test_photo_normalized_without_changing_source(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'photo.jpg'
            Image.new('RGB', (2000, 1000), 'red').save(source, progressive=True)
            before = source.read_bytes()
            with push.prepared_image(source) as result:
                with Image.open(result) as image:
                    self.assertEqual(image.size, (960, 480))
                    self.assertFalse(image.info.get('progressive', False))
                self.assertLess(result.stat().st_size, len(before))
            self.assertEqual(source.read_bytes(), before)
            self.assertFalse(result.exists())

    def test_original_and_nonimage_are_unchanged(self):
        for source, original in [(Path('photo.png'), True), (Path('document.txt'), False)]:
            with push.prepared_image(source, original) as result:
                self.assertEqual(source, result)

    def test_png_transparency(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'photo.png'
            photo = Image.new('RGBA', (1200, 600), (10, 20, 30, 0))
            metadata = Image.Exif()
            metadata[315] = 'Synthetic private metadata'
            photo.save(source, exif=metadata)
            with push.prepared_image(source) as result:
                with Image.open(result) as image:
                    self.assertEqual(image.size, (960, 480))
                    self.assertEqual(image.getpixel((0, 0))[3], 0)
                    self.assertFalse(image.getexif())


if __name__ == '__main__':
    unittest.main()
