import {render,screen,waitFor,cleanup} from '@testing-library/react';
import {afterEach,describe,it,expect,vi} from 'vitest';
import App from './App';
const mock=vi.hoisted(()=>({mode:'workout',view:'screen',phase:'WEIGHT_CONFIRMATION',notice:null as string|null}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn((name:string)=>Promise.resolve(name==='get_app_mode'?mock.mode:{view:mock.view,previewOnly:true,lastError:null,notice:mock.notice}))}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(()=>Promise.resolve(()=>{}))}));
vi.mock('./useSnapshot',()=>({useSnapshot:()=>({phase:mock.phase,remainingSeconds:60,prescription:{exercise:'squat',kind:'REP',targetReps:5,targetSeconds:0,defaultWeight:0},progress:null,capacityUsed:0,capacityLimit:20,rotation:[],pointer:0})}));
afterEach(()=>{cleanup();mock.notice=null;});
describe('CLI-only displays',()=>{
 it('shows the end-of-day warning without adding controls',async()=>{
  Object.assign(mock,{mode:'workout',view:'screen',phase:'EXERCISE_REQUIRED',notice:'Your workday is ending soon. Start when ready: rfp start'});
  const {container}=render(<App/>);
  expect(await screen.findByRole('status')).toHaveTextContent('Your workday is ending soon');
  expect(container.querySelectorAll('button,input,a')).toHaveLength(0);
 });
 for(const [mode,view,phase] of [['workout','screen','WEIGHT_CONFIRMATION'],['debug','screen','WORKOUT_ACTIVE'],['debug','camera','CODING'],['debug','video','CODING']]){
  it(`${mode}/${view} has no interactive controls`,async()=>{
   Object.assign(mock,{mode,view,phase});const {container}=render(<App/>);
   await waitFor(()=>expect(screen.queryByText('Connecting to RFP…')).not.toBeInTheDocument());
   expect(container.querySelectorAll('button,input,select,textarea,a,summary,[tabindex]')).toHaveLength(0);
   if(phase==='WEIGHT_CONFIRMATION')expect(screen.getByText('rfp finish --weight 0')).toBeInTheDocument();
  });
 }
});
