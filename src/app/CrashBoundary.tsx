import {Component,type ErrorInfo,type ReactNode} from "react";
import {getDiagnosticsReport,recordClientDiagnostic} from "../shared/backend";

interface State{error?:Error;copyStatus:string}

export class CrashBoundary extends Component<{children:ReactNode},State>{
 state:State={copyStatus:""};
 static getDerivedStateFromError(error:Error):State{return{error,copyStatus:""}}
 componentDidCatch(error:Error,info:ErrorInfo){void recordClientDiagnostic("react.render",error.message,`${error.stack||""}\n${info.componentStack||""}`)}
 private copy=async()=>{try{const report=await getDiagnosticsReport();await navigator.clipboard.writeText(report.compactText);this.setState({copyStatus:"Diagnostic copied for Discord"})}catch(error){this.setState({copyStatus:`Copy failed: ${String(error)}`})}};
 render(){if(!this.state.error)return this.props.children;return <main className="crash-screen"><section><span className="eyebrow">Application error</span><h1>The workspace encountered a problem</h1><p>The error was saved to the diagnostic log. Copy the compact report and paste it into Discord so the failure can be investigated.</p><pre>{this.state.error.message}</pre><div className="button-row"><button className="primary" onClick={()=>void this.copy()}>Copy Discord diagnostic</button><button onClick={()=>location.reload()}>Restart interface</button></div><strong role="status">{this.state.copyStatus}</strong></section></main>}
}
