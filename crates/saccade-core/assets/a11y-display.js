/* Machado, Oliveira & Fernandes (2009), doi:10.1109/TVCG.2009.113.
 * Severity 1.0 matrices; SVG linearRGB performs sRGB decode/encode around
 * the matrix. Display aid only: PRE-CHECK, not certification/compliance. */
(function () {
  'use strict';
  var UI=window.__saccadeUI,ns='http:'+'//www.w3.org/2000/svg';
  UI.cvdChoices=[['protan','Simulate: protan'],['deutan','Simulate: deutan'],['tritan','Simulate: tritan']];
  UI.installCvd=function(){
    if(document.getElementById('ch-protan'))return;
    var svg=document.createElementNS(ns,'svg');svg.setAttribute('width','0');svg.setAttribute('height','0');svg.setAttribute('aria-hidden','true');svg.style.position='absolute';
    var matrices=[
      [0.152286,1.052583,-0.204868,0.114503,0.786281,0.099216,-0.003882,-0.048116,1.051998],
      [0.367322,0.860646,-0.227968,0.280085,0.672501,0.047413,-0.011820,0.042940,0.968881],
      [1.255528,-0.076749,-0.178779,-0.078411,0.930809,0.147602,0.004733,0.691367,0.303900]
    ];
    UI.cvdChoices.forEach(function(c,i){var f=document.createElementNS(ns,'filter');f.id='ch-'+c[0];f.setAttribute('color-interpolation-filters','linearRGB');var m=document.createElementNS(ns,'feColorMatrix');m.setAttribute('type','matrix');var a=matrices[i];m.setAttribute('values',a.slice(0,3).concat([0,0],a.slice(3,6),[0,0],a.slice(6,9),[0,0,0,0,0,1,0]).join(' '));f.appendChild(m);svg.appendChild(f);});
    document.body.appendChild(svg);
  };
}());
