(function () {
  'use strict';
  var D = JSON.parse(document.getElementById('precheck-data').textContent);
  function el(t, text, parent) { var e=document.createElement(t); if(text!=null)e.textContent=text; if(parent)parent.appendChild(e); return e; }
  function image(path, parent, label) { var img=el('img',null,parent); img.src=path.split('/').map(encodeURIComponent).join('/');img.alt=label;return img; }
  var summary=document.getElementById('summary');summary.textContent=D.verdict+' — '+(D.frames?D.frames.length+' frames':D.images.length+' images');summary.className=D.verdict;
  (D.warnings||[]).forEach(function(w){el('p',w,document.getElementById('warnings'));});
  document.getElementById('thresholds').textContent=JSON.stringify(D.thresholds,null,2);
  var nav=document.getElementById('timeline'),detail=document.getElementById('detail');
  function reset(b) { detail.textContent='';Array.from(nav.children).forEach(function(x){x.setAttribute('aria-pressed',String(x===b));}); }
  (D.segments||D.images||[]).forEach(function(s){
    var b=el('button',D.frames?s.verdict+' '+s.kind+' '+s.start_seconds.toFixed(3)+'–'+s.end_seconds.toFixed(3)+'s':s.name+' '+s.verdict,nav);b.type='button';b.setAttribute('aria-pressed','false');
    b.addEventListener('click',function(){reset(b);
      if(D.frames){
        el('h2',s.kind+' — '+s.verdict,detail);el('p','Peak '+s.peak_flash_rate+' flashes/s; '+s.flashing_area_percent.toFixed(3)+'% screen area. Pre-check only.',detail);
        image(s.heatmap,detail,'Risk heatmap: orange = affected pixels').className='heat';
        var frames=el('div',null,detail);frames.className='frames';
        // Static frames only: this report never plays a flashing sequence.
        D.frames.slice(s.start_frame,s.end_frame+1).forEach(function(f){var fig=el('figure',null,frames);image(f.image,fig,'Frame '+f.index);el('figcaption','Frame '+f.index+' ('+f.timestamp.toFixed(3)+'s)',fig);});
      }else{
        el('h2',s.name+' — '+s.verdict,detail);
        var select=el('select',null,detail);select.setAttribute('aria-label','Simulate colour vision');
        [['original','Original'],['protanopia','Simulate: protan'],['deuteranopia','Simulate: deutan'],['tritanopia','Simulate: tritan'],['protanomaly','Protanomaly 1.0'],['deuteranomaly','Deuteranomaly 1.0'],['tritanomaly','Tritanomaly 1.0']].forEach(function(k){var o=el('option',k[1],select);o.value=k[0];});
        var pic=el('div',null,detail);pic.className='picture';var im=image(s.original,pic,'Original image');
        (s.findings||[]).forEach(function(f){var box=el('div',null,pic);box.className='box';var r=f.rect;box.style.cssText='left:'+r[0]*100+'%;top:'+r[1]*100+'%;width:'+r[2]*100+'%;height:'+r[3]*100+'%';box.title=f.message;el('p',f.severity+': '+f.message,detail);});
        var heat=image(s.heatmaps.deuteranopia,detail,'Information-loss heatmap: deutan');heat.className='heat';
        select.addEventListener('change',function(){im.src=select.value==='original'?s.original:s.simulations[select.value];im.alt=select.options[select.selectedIndex].text;heat.src=s.heatmaps[select.value]||s.heatmaps.deuteranopia;});
        (s.contrast||[]).forEach(function(c){el('p',c.name+': '+c.verdict+'; '+(c.ratio==null?'unmeasurable':c.ratio.toFixed(3)+':1')+'; threshold '+c.threshold+':1. '+c.note,detail);});
        (s.proposals||[]).forEach(function(p){el('p','PROPOSAL ONLY (confirm in config): '+p.kind+' '+JSON.stringify(p.rect),detail);});
      }
    });
  });
  if(nav.firstChild)nav.firstChild.click();else el('p','No risk segments found in this pre-check.',detail);
}());
