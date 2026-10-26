import {EditorState, Compartment} from '@codemirror/state';
import {EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection} from '@codemirror/view';
import {defaultKeymap, history, historyKeymap} from '@codemirror/commands';
import {searchKeymap} from '@codemirror/search';
import {syntaxHighlighting, StreamLanguage, LanguageSupport} from '@codemirror/language';
import {classHighlighter, highlightTree} from '@lezer/highlight';
import {rust} from '@codemirror/lang-rust';
import {javascript} from '@codemirror/lang-javascript';
import {json} from '@codemirror/lang-json';
import {css} from '@codemirror/lang-css';
import {html} from '@codemirror/lang-html';
import {markdown} from '@codemirror/lang-markdown';
import {python} from '@codemirror/lang-python';
import {go} from '@codemirror/lang-go';
import {java} from '@codemirror/lang-java';
import {cpp} from '@codemirror/lang-cpp';
import {sql} from '@codemirror/lang-sql';
import {yaml} from '@codemirror/lang-yaml';
import {xml} from '@codemirror/lang-xml';
import {shell} from '@codemirror/legacy-modes/mode/shell';
import {toml} from '@codemirror/legacy-modes/mode/toml';

export function languageForPath(path: string): string | null {
  const name = path.split('/').pop()!.toLowerCase();
  if (['.bashrc','.zshrc','.profile'].includes(name)) return 'shell';
  const ext = name.split('.').pop();
  return ({rs:'rust',js:'javascript',mjs:'javascript',cjs:'javascript',jsx:'jsx',ts:'typescript',tsx:'tsx',json:'json',css:'css',html:'html',htm:'html',md:'markdown',py:'python',go:'go',java:'java',c:'cpp',h:'cpp',cpp:'cpp',hpp:'cpp',cc:'cpp',sql:'sql',yaml:'yaml',yml:'yaml',xml:'xml',svg:'xml',sh:'shell',bash:'shell',zsh:'shell',toml:'toml'} as Record<string,string>)[ext!] ?? null;
}
function language(name: string | null): LanguageSupport | null {
  switch(name) {
    case 'rust':return rust(); case 'javascript':return javascript(); case 'typescript':return javascript({typescript:true}); case 'jsx':return javascript({jsx:true}); case 'tsx':return javascript({typescript:true,jsx:true});
    case 'json':return json(); case 'css':return css(); case 'html':return html(); case 'markdown':return markdown(); case 'python':return python(); case 'go':return go(); case 'java':return java(); case 'cpp':return cpp(); case 'sql':return sql(); case 'yaml':return yaml(); case 'xml':return xml();
    case 'shell':return new LanguageSupport(StreamLanguage.define(shell)); case 'toml':return new LanguageSupport(StreamLanguage.define(toml)); default:return null;
  }
}
export function highlightLines(name: string, lines: string[]) {
  if (lines.length > 5000) throw new Error('Highlight at most 5000 lines per call');
  const support = language(name);
  const result: {start:number,end:number,class:string}[][] = lines.map(()=>[]);
  if (!support) return result;
  const text = lines.join('\n');
  const offsets = [0]; for (const line of lines) offsets.push(offsets.at(-1)! + line.length + 1);
  highlightTree(support.language.parser.parse(text), classHighlighter, (from,to,cls)=> {
    // Each token may span multiple lines; offsets intentionally use UTF-16.
    let lo=0, hi=lines.length; while(lo+1<hi){const mid=(lo+hi)>>1;if(offsets[mid]<=from)lo=mid;else hi=mid;}
    for(let i=lo;i<lines.length && offsets[i]<to;i++) {
      const start=Math.max(from,offsets[i])-offsets[i], end=Math.min(to,offsets[i]+lines[i].length)-offsets[i];
      if(end>start) result[i].push({start,end,class:cls});
    }
  });
  return result;
}
const views = new Map<string,{view:EditorView, editable:Compartment, lang:Compartment,path:string|undefined,readOnly:boolean,lastGoto:string}>();
export function mount(id:string, host:HTMLElement, options:any, send:(event:any)=>void) {
  destroy(id); const editable=new Compartment(),lang=new Compartment();
  const view = new EditorView({parent:host,state:EditorState.create({doc:options.value,extensions:[lineNumbers(),history(),drawSelection(),highlightActiveLine(),syntaxHighlighting(classHighlighter),lang.of(language(languageForPath(options.path??''))??[]),editable.of([EditorState.readOnly.of(options.read_only),EditorView.editable.of(!options.read_only)]),keymap.of([{key:'Mod-s',run:()=>{send({kind:'save'});return true;}},...defaultKeymap,...historyKeymap,...searchKeymap]),EditorView.updateListener.of(update=>{
    if(update.docChanged)send({kind:'change',value:update.state.doc.toString()});
    if(update.selectionSet){const s=update.state.selection.main;send({kind:'selection',start_line:update.state.doc.lineAt(s.from).number,end_line:update.state.doc.lineAt(s.to).number,text:update.state.sliceDoc(s.from,s.to)});}
  })]})});
  host.previousElementSibling?.setAttribute('hidden',''); views.set(id,{view,editable,lang,path:options.path,readOnly:options.read_only,lastGoto:JSON.stringify(options.goto)}); goto(id,options.goto);
}
export function update(id:string, options:any) {
  const entry=views.get(id);if(!entry)return;
  const {view,editable,lang}=entry;
  if(options.value!==view.state.doc.toString())view.dispatch({changes:{from:0,to:view.state.doc.length,insert:options.value}});
  if(entry.readOnly!==options.read_only){view.dispatch({effects:editable.reconfigure([EditorState.readOnly.of(options.read_only),EditorView.editable.of(!options.read_only)])});entry.readOnly=options.read_only;}
  if(entry.path!==options.path){view.dispatch({effects:lang.reconfigure(language(languageForPath(options.path??''))??[])});entry.path=options.path;}
  const target=JSON.stringify(options.goto);if(entry.lastGoto!==target){goto(id,options.goto);entry.lastGoto=target;}
}
export function goto(id:string,target:any) {
  if(!target)return;const view=views.get(id)?.view;if(!view)return;
  const line=view.state.doc.line(Math.max(1,Math.min(target.line,view.state.doc.lines)));
  const position=Math.min(line.to,line.from+Math.max(0,(target.column??1)-1));
  view.dispatch({selection:{anchor:position},effects:EditorView.scrollIntoView(position,{y:'center'})});view.focus();
}
export function destroy(id:string){views.get(id)?.view.destroy();views.delete(id);}
