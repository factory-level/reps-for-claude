import type { Metadata } from 'next';
import './style.css';
export const metadata:Metadata={title:'RFP — reps for prompts',description:'A little movement between prompts. Share a workout, cheer someone on, and get back to making things.'};
export default function Layout({children}:{children:React.ReactNode}){return <html lang="en"><body>{children}</body></html>;}
