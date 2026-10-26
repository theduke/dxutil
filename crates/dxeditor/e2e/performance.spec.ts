import { test, expect } from '@playwright/test';
import { writeFile } from 'node:fs/promises';
import { open, editor, caret, state } from './helpers.js';

for (const count of [2000,10000]) test(`typing and mount metrics at ${count} blocks`,async ({page,browserName},testInfo) => {
  test.skip(browserName!=='chromium','Performance comparisons use one browser and machine.');
  test.setTimeout(120_000);
  await open(page,`/perf?blocks=${count}`);
  await expect.poll(async () => (await state(page)).mountMs).toBeGreaterThan(0);
  const mountMs=(await state(page)).mountMs;
  await expect(editor(page).locator('.dx-block')).toHaveCount(count);
  await caret(page,editor(page).locator('[data-content-kind=paragraph]').last());
  await editor(page).evaluate(element=>element.setAttribute('data-measure-timings','true'));
  const samples:number[]=[];
  const stages:{totalMs:number,engineMs:number,renderMs:number,callbackMs:number,patchMs:number,mediaMs:number,selectionMs:number}[]=[];
  for(let index=0;index<30;index++) {
    const elapsed=await editor(page).evaluate(element=>{
      const start=performance.now();
      element.dispatchEvent(new InputEvent('beforeinput',{bubbles:true,cancelable:true,inputType:'insertText',data:'x'}));
      return {totalMs:performance.now()-start,engineMs:Number(element.getAttribute('data-perf-engine-ms')),renderMs:Number(element.getAttribute('data-perf-render-ms')),callbackMs:Number(element.getAttribute('data-perf-callback-ms')),patchMs:Number(element.getAttribute('data-perf-patch-ms')),mediaMs:Number(element.getAttribute('data-perf-media-ms')),selectionMs:Number(element.getAttribute('data-perf-selection-ms'))};
    });
    samples.push(elapsed.totalMs);
    stages.push(elapsed);
  }
  await expect(editor(page).locator('[data-content-kind=paragraph]').last()).toContainText('x'.repeat(30));
  samples.sort((a,b)=>a-b);
  const p95=(values:number[])=>values.sort((a,b)=>a-b)[Math.floor(values.length*.95)];
  const result={buildProfile:process.env.DXEDITOR_PROFILE||'debug',stageP95Ms:{engine:p95(stages.map(s=>s.engineMs)),render:p95(stages.map(s=>s.renderMs)),callback:p95(stages.map(s=>s.callbackMs)),patch:p95(stages.map(s=>s.patchMs)),media:p95(stages.map(s=>s.mediaMs)),selection:p95(stages.map(s=>s.selectionMs))},stageSamples:stages,blocks:count,islands:50,mountMs,typingP95Ms:samples[Math.floor(samples.length*.95)],typingToDomP95Ms:p95(stages.map(s=>Math.max(0,s.totalMs-s.callbackMs))),typingSamplesMs:samples,budgets:{mountMs:count===2000?400:null,typingP95Ms:16,callbackP95Ms:count===2000?30:null}};
  const artifact=testInfo.outputPath(`performance-${count}.json`);
  await writeFile(artifact,JSON.stringify(result,null,2));
  await testInfo.attach(`performance-${count}.json`,{path:artifact,contentType:'application/json'});
  // Report measurements faithfully; hardware-dependent budgets are evaluated in
  // the validation report, rather than hiding slow results behind skipped tests.
  expect(result.typingP95Ms).toBeGreaterThan(0);
});
