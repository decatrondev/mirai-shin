// Fondo de muestra hecho en código (sin arte de terceros): ruido fbm deformado sobre cuatro colores.
// `e` es el encendido: 0 = apagado, 1 = cubre toda la pantalla desde el centro (la mecha, 芯).
export const VERT = `#version 300 es
const vec2 v[3] = vec2[3](vec2(-1.,-1.), vec2(3.,-1.), vec2(-1.,3.));
void main(){ gl_Position = vec4(v[gl_VertexID], 0., 1.); }`

export const FRAG = `#version 300 es
precision highp float;
out vec4 o;
uniform vec2 r; uniform float t; uniform float b; uniform float e;
uniform vec3 c0, c1, c2, c3;
float h(vec2 p){ p = fract(p*vec2(123.34,456.21)); p += dot(p, p+45.32); return fract(p.x*p.y); }
float n(vec2 p){ vec2 i=floor(p), f=fract(p); f=f*f*(3.-2.*f);
  return mix(mix(h(i),h(i+vec2(1,0)),f.x), mix(h(i+vec2(0,1)),h(i+vec2(1,1)),f.x), f.y); }
float fbm(vec2 p){ float v=0., a=.5; mat2 m=mat2(1.6,1.2,-1.2,1.6);
  for(int i=0;i<5;i++){ v+=a*n(p); p=m*p; a*=.5; } return v; }
void main(){
  vec2 p = (gl_FragCoord.xy - .5*r) / r.y;
  vec2 q = vec2(fbm(p*1.6 + t*.03), fbm(p*1.6 + vec2(5.2,1.3) - t*.025));
  vec2 w = vec2(fbm(p*1.6 + 3.*q + vec2(1.7,9.2) + t*.05), fbm(p*1.6 + 3.*q + vec2(8.3,2.8) + t*.04));
  float f = fbm(p*1.6 + 3.*w);
  vec3 col = mix(c0, c1, clamp(f*f*2.2, 0., 1.));
  col = mix(col, c2, clamp(pow(w.x, 2.4)*1.6, 0., 1.)*.75);
  col = mix(col, c3, clamp(pow(f, 4.)*3., 0., 1.));
  float rad = e*1.4;
  float lit = smoothstep(rad, rad - .35, length(p));
  o = vec4(mix(c0*.6, col*b, lit), 1.);
}`
