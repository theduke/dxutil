import {describe,it,expect} from 'vitest';
import {languageForPath,highlightLines} from './index';
describe('languages and tokens',()=>{
  it('detects names and extensions',()=>{expect(languageForPath('src/app.TSX')).toBe('tsx');expect(languageForPath('.bashrc')).toBe('shell');expect(languageForPath('log.txt')).toBe(null);});
  it('uses UTF-16 offsets and splits multiline tokens',()=>{const lines=['const x = "😀";','/* hello','world */'];const result=highlightLines('javascript',lines);expect(result[0].some(t=>t.start===10&&t.end===14)).toBe(true);expect(result[1][0].end).toBe(lines[1].length);expect(result[2][0].start).toBe(0);});
  it('bounds work and tolerates plain text',()=>{expect(()=>highlightLines('rust',Array(5001).fill(''))).toThrow();expect(highlightLines('unknown',['hi'])).toEqual([[]]);});
});
