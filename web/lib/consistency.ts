export type RoutineDay={date:string;completed:number;target:number;updatedAt:string};
export type HeatDay={date:string;ratio:number|null};
export type ConsistencyEntry={id:string;nickname:string;location:string|null;heatmap:HeatDay[];delta:number|null;latestPercent:number|null;demo?:boolean};
export type ConsistencyBoard={entries:ConsistencyEntry[];endDate:string};
export function dateBefore(end:string,n:number){const d=new Date(end+'T12:00:00Z');d.setUTCDate(d.getUTCDate()-n);return d.toISOString().slice(0,10);}
export function progress(days:RoutineDay[],endDate:string){
 const mapped=new Map(days.filter(d=>d.target>0).map(d=>[d.date,Math.min(1,Math.max(0,d.completed/d.target))]));
 const heatmap=Array.from({length:28},(_,i)=>{const date=dateBefore(endDate,27-i);return {date,ratio:mapped.get(date)??null};});
 // Compare tracked days in two completed seven-day windows; today's partial work is excluded.
 const average=(start:number,end:number)=>{const values=heatmap.slice(start,end).flatMap(d=>d.ratio===null?[]:[d.ratio]);return values.length?values.reduce((a,b)=>a+b,0)/values.length:null;};
 const previous=average(13,20),recent=average(20,27);
 const latest=heatmap.filter(d=>d.ratio!==null).at(-1)?.ratio;
 return {heatmap,delta:previous===null||recent===null?null:Math.round((recent-previous)*100),latestPercent:latest==null?null:Math.round(latest*100)};
}
export function mergeDays(existing:RoutineDay[],incoming:RoutineDay[],today:string){
 const byDate=new Map<string,RoutineDay>();
 for(const day of [...existing,...incoming]){
  if(day.date<dateBefore(today,27)||day.date>dateBefore(today,-1))continue;
  const prior=byDate.get(day.date);if(!prior||day.updatedAt>=prior.updatedAt)byDate.set(day.date,day);
 }
 return [...byDate.values()].sort((a,b)=>a.date.localeCompare(b.date)).slice(-28);
}
