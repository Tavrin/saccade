"""Invented fixtures for the versioned region rule; no responses or provider calls."""
import copy
from pathlib import Path
import tempfile
import unittest
from PIL import Image
from score import region_matches, REGION_MIN_IOU, REGION_MIN_COVERAGE
from dev_policy import assertion_correct, normalize, task_evidence


class GeometryToleranceTests(unittest.TestCase):
    def test_thresholds_are_inclusive_and_independent(self):
        self.assertEqual((REGION_MIN_IOU,REGION_MIN_COVERAGE),(.8,.9))
        box=lambda v:dict(geometry=dict(type='box',pixels=v))
        region=[10,10,100,100]
        self.assertTrue(region_matches(box([10,10,90,100]),region))
        self.assertFalse(region_matches(box([10,10,89.999,100]),region))
        self.assertTrue(region_matches(box([10,10,125,100]),region))
        self.assertFalse(region_matches(box([10,10,125.001,100]),region))
        self.assertFalse(region_matches(box([11,10,99,100]),region,'strict'))
        self.assertTrue(region_matches(box([9,9,102,102]),region,'strict'))
        for coords in ([10,10,0,100], [float('nan'),10,100,100],[-1,10,100,100]):
            self.assertFalse(region_matches(box(coords),region))
        self.assertFalse(region_matches(dict(geometry=dict(type='point',pixels=[10,10])),region))
        with self.assertRaises(ValueError):region_matches(box(region),region,'typo')

    def test_thin_region_jitter_is_not_automatically_accepted(self):
        # Image-relative percentages never override the specified region thresholds.
        self.assertFalse(region_matches(dict(geometry=dict(type='box',pixels=[0,1.6,100,20.8])),[0,0,100,24]))

    def test_jitter_cannot_hide_changed_pixels_or_import_decorative_changes(self):
        case=dict(task='explain',dimensions=[200,200],target=[20,20,100,100],label='X',before='b.png',after='a.png',exclusions=[])
        truth=dict(rendered_witness=dict(label_pixels=[],before_label_pixels=[],label_complete=False,before_label_complete=False))
        o=dict(image_role='after',kind='appearance',statement='appearance:changed',geometry=dict(type='box',pixels=[21,21,98,98]),evidence_refs=['after:R0'])
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp)
            before=Image.new('RGB',(200,200),'white');after=before.copy();after.putpixel((20,20),(0,0,0))
            before.save(directory/'b.png');after.save(directory/'a.png')
            self.assertTrue(assertion_correct(o,truth,case,directory))
            self.assertFalse(assertion_correct(dict(o,statement='appearance:unchanged'),truth,case,directory))
            self.assertFalse(assertion_correct(o,truth,case,directory,'strict'))
            after=before.copy();after.putpixel((19,19),(0,0,0));after.save(directory/'a.png')
            o['geometry']['pixels']=[19,19,102,102]
            self.assertFalse(assertion_correct(o,truth,case,directory))
            self.assertTrue(assertion_correct(dict(o,statement='appearance:unchanged'),truth,case,directory))
            self.assertFalse(assertion_correct(dict(o,evidence_refs=['after:R0','after:R1']),truth,case,directory))
            self.assertFalse(task_evidence([dict(o,statement='appearance:unchanged')]*2,case,truth,directory))


if __name__=='__main__':unittest.main()
