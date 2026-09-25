import type { Metadata } from 'next';
import './style.css';
export const metadata:Metadata={title:'RFP — reps for prompts',description:'Small workouts. A little more often. Share your consistency between prompts.'};
export default function Layout({children}:{children:React.ReactNode}){return <html lang="en"><body>{children}</body></html>;}
