"""Offline parity contracts; fixtures establish reducer behavior, never model quality."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import numpy as np
from PIL import Image
import image_quality as quality
import judge_gate


def fixture():
    config=dict(providers=[dict(provider='one',model='model',revision='absent'),dict(provider='two',model='other',revision='revision')],
        evaluations=[dict(source='a',view_id='view',kind=kind,sample_ids=['first','second'],order_count=2) for kind in ('still','motion')],
        threshold=7,required_cues=['cue'],criteria=[dict(id='criterion',minimum=0,maximum=3,threshold=2)],forbidden=['condition'],scorecard=dict(floor=2,mean=2.3,conjunction=True))
    rows=[]
    for e in config['evaluations']:
        for p in config['providers']:
            for sample,score in zip(e['sample_ids'],[7,9]):
                for order in (0,1):
                    rows.append(dict(**p,returned_model=p['model'],returned_revision=p['revision'],source=e['source'],view_id=e['view_id'],kind=e['kind'],sample_id=sample,
                        order=order,outcome='scored',score=score,replay=False,preferred_source='a',cues=[dict(cue='cue',state='present',timestamps_s=[0])],
                        criteria=[dict(id='criterion',score=3)],forbidden=[dict(condition='condition',state='absent')]))
    return rows,config


class JudgeGate(unittest.TestCase):
    def test_oracle_perfect_wrong_gate_selftest(self):
        result=judge_gate.selftest()
        self.assertEqual(result['status'],'PASS',result)

    def test_exact_per_provider_per_kind_medians_and_even_rule(self):
        rows,config=fixture();result=judge_gate.reduce(rows,config)
        self.assertTrue(result['pass'])
        self.assertEqual(len(result['groups']),4)
        self.assertEqual([g['median'] for g in result['groups']],[8]*4)
        self.assertEqual(result['trusted_for'],[])
        rows[-1]['score']=1
        self.assertFalse(judge_gate.reduce(rows,config)['pass'])

    def test_complete_denominator_and_order_outcomes(self):
        rows,config=fixture()
        for variant in (rows[:-1],rows+rows[:1],[] ):
            self.assertFalse(judge_gate.reduce(variant,config)['pass'])
        rows[1]['preferred_source']='b'
        group=judge_gate.reduce(rows,config)['groups'][0]
        self.assertEqual(group['samples'][0]['status'],'inconsistent')
        self.assertIsNone(group['median'])
        self.assertFalse(group['pass'])
        rows[1]['outcome']='abstain';rows[1]['score']=None
        self.assertEqual(judge_gate.reduce(rows,config)['groups'][0]['samples'][0]['status'],'incomplete')

    def test_cues_forbidden_floor_mean_conjunction_and_revision(self):
        rows,config=fixture()
        for field,value in [('cues',[]),('cues',[dict(cue='cue',state='absent',timestamps_s=[0])]),
            ('forbidden',[dict(condition='condition',state='unknown')]),('criteria',[dict(id='criterion',score=2)]),
            ('returned_revision','wrong'),('replay',True)]:
            wrong=copy.deepcopy(rows);wrong[0][field]=value
            self.assertFalse(judge_gate.reduce(wrong,config)['pass'],field)
        unknown=copy.deepcopy(rows);del unknown[0]['replay']
        self.assertFalse(judge_gate.reduce(unknown,config)['pass'])
        config['scorecard']['mean']=None
        wrong=copy.deepcopy(rows);wrong[0]['criteria'][0]['score']=2
        self.assertTrue(judge_gate.reduce(wrong,config)['pass'])
        config['criteria'][0]['threshold']=3
        self.assertFalse(judge_gate.reduce(wrong,config)['pass'])
        config['scorecard']['conjunction']=False
        self.assertTrue(judge_gate.reduce(wrong,config)['pass'])

    def test_no_scale_coercion_or_missing_configuration(self):
        rows,config=fixture()
        config['criteria'].append(dict(id='other',minimum=1,maximum=10,threshold=7))
        with self.assertRaises(ValueError):judge_gate.reduce(rows,config)
        del config['providers']
        with self.assertRaises(ValueError):judge_gate.reduce(rows,config)

    def test_strict_ci_exit_and_markdown(self):
        rows,config=fixture()
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'config').write_text(json.dumps(config));(root/'scores').write_text('\n'.join(json.dumps(r) for r in rows))
            cmd=[sys.executable,str(Path(judge_gate.__file__)), '--scores',str(root/'scores'),'--config',str(root/'config'),'--out',str(root/'out'),'--markdown',str(root/'report'),'--strict']
            self.assertEqual(subprocess.run(cmd,capture_output=True).returncode,0)
            (root/'out').unlink();(root/'report').unlink();rows[0]['score']=None
            (root/'scores').write_text('\n'.join(json.dumps(r) for r in rows))
            self.assertEqual(subprocess.run(cmd,capture_output=True).returncode,1)
            self.assertIn('FAIL',(root/'report').read_text())


class ImageQuality(unittest.TestCase):
    def test_grid_percentile_buckets_edges_and_rounding(self):
        stripes=np.zeros((900,1600,3),dtype=np.uint8)
        for x in range(0,1600,20):stripes[:,x+10:x+20]=255
        m=quality.compute(stripes)
        self.assertEqual((m['grid_samples'],m['luminance_contrast'],m['colour_entropy_bits'],m['dominant_colour_share'],m['edge_density'],m['clipped_share_outside_mask']),
                         (14400,255,1,.5,1,.5))
        flat=quality.compute(np.full((180,320,3),128,dtype=np.uint8))
        failed=[k for k,v in quality.absolute(flat,{ }).items() if not v['pass']]
        self.assertEqual(failed,['luminance_contrast','colour_entropy_bits','dominant_colour_share','edge_density'])
        self.assertTrue(quality.absolute(flat,dict(exposure_band=[100,140]))['luminance_p50_in_band']['pass'])

    def test_mask_full_pixel_palette_and_cie76(self):
        image=np.full((100,200,3),60,dtype=np.uint8);image[:,100:]=255
        mask=np.zeros((100,200),dtype=bool);mask[:,100:]=True
        self.assertEqual(quality.compute(image,mask)['clipped_share_outside_mask'],0)
        mask[:,100:150]=False
        self.assertAlmostEqual(quality.compute(image,mask)['clipped_share_outside_mask'],1/3,places=3)
        lab=quality.srgb_to_lab(np.array([[255,0,0],[255,255,255],[0,0,0]],dtype=np.uint8))
        np.testing.assert_allclose(lab,[[53.24,80.09,67.20],[100,0,0],[0,0,0]],atol=.05)
        pal=[dict(role='base',hex='#1a3a5c',rgb=[26,58,92]),dict(role='accent',hex='#ff2bd6',rgb=[255,43,214])]
        image=np.zeros((40,40,3),dtype=np.uint8);image[:10]=pal[0]['rgb'];image[10:20]=pal[1]['rgb'];image[20:30]=[40,200,40]
        self.assertEqual(quality.compute(image,palette=pal)['palette_coverage'],.667)

    def test_all_relative_formulas_new_absolute_failures_and_configuration(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            rng=np.random.default_rng(7);noise=rng.integers(0,254,size=(90,160,3),dtype=np.uint8)
            Image.fromarray(noise).save(root/'base.png');Image.fromarray(noise).save(root/'same.png')
            Image.new('RGB',(160,90),(128,128,128)).save(root/'flat.png')
            same=quality.evaluate(root/'same.png',{},root/'base.png')
            self.assertTrue(same['pass']);self.assertFalse(same['flagged'])
            self.assertEqual([g['value'] for g in same['regression'].values()],[1,1,1,1,0,0,0,0])
            flat=quality.evaluate(root/'flat.png',{},root/'base.png')
            self.assertFalse(flat['pass']);self.assertTrue(flat['flagged'])
            self.assertEqual(len(flat['new_absolute_failures']),4)
            self.assertIn('colour_entropy_delta_abs',flat['failed_regression_gates'])
            relaxed=dict(absolute={k:['>=',0] for k in quality.GATES if k!='palette_coverage'})
            self.assertTrue(quality.evaluate(root/'flat.png',relaxed)['pass'])
            with self.assertRaises(ValueError):quality.evaluate(root/'flat.png',dict(absolute={'unknown':['>=',0]}))
            Image.new('L',(10,10)).save(root/'mask.png')
            with self.assertRaises(ValueError):quality.evaluate(root/'flat.png',{},mask=root/'mask.png')
            # Baseline absolute failure remains a failure, but is not new.
            repeated=quality.evaluate(root/'flat.png',{},root/'flat.png')
            self.assertEqual(repeated['new_absolute_failures'],[])
            self.assertFalse(repeated['pass'])

if __name__=='__main__':unittest.main()
