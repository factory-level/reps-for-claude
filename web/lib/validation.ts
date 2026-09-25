import { z } from 'zod';
const date = z.string().regex(/^\d{4}-\d{2}-\d{2}$/).refine(s => !isNaN(Date.parse(s)) && new Date(s).toISOString().slice(0,10) === s);
export const activity = z.object({nickname:z.string().trim().min(1).max(32),exercise:z.string().trim().min(1).max(60),quantity:z.number().positive().max(100000),unit:z.enum(['reps','seconds','minutes']),note:z.string().trim().max(280).default(''),website:z.string().max(0).optional()}).refine(v => v.unit !== 'reps' || Number.isInteger(v.quantity), {message:'Reps must be a whole number'});
export const subscriber = z.object({email:z.email().max(254).transform(s=>s.toLowerCase()),website:z.string().max(0).optional()});
export const workout = z.object({id:z.string().regex(/^[a-zA-Z0-9-]{1,80}$/),date,exercise:z.string().min(1).max(100),kind:z.enum(['REP','CONTINUOUS']),reps:z.number().int().min(0).max(100000),seconds:z.number().min(0).max(604800),weight:z.number().min(0).max(100000),verified:z.boolean(),recordedAt:z.iso.datetime().nullable(),weightUnit:z.enum(['lb','kg']).nullable()}).refine(v=>v.kind==='REP'?v.reps>0:v.seconds>0,{message:'A completed workout must include reps or duration'});
export const syncBody=z.object({records:z.array(workout).max(100)});
export function query(url: URL) {
 return z.object({from:date.optional(),to:date.optional(),cursor:z.coerce.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).default(0),limit:z.coerce.number().int().min(1).max(100).default(50)}).parse(Object.fromEntries(url.searchParams));
}
export type Activity = {id:string; nickname:string; exercise:string; quantity:number; unit:string; note:string; created_at:string; featured:boolean; cheers:number};
