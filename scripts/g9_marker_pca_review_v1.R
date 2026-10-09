#!/usr/bin/env Rscript
root<-"/home/kingrider/GSE89223_download"
a<-file.path(root,"audits/G9_closure_20261009")
stopifnot(file.exists(file.path(a,"marker_annotation_v19.csv")))
ann<-read.csv(file.path(a,"marker_annotation_v19.csv"))
qc<-read.csv(file.path(a,"fastqc_salmon_samples_v1.csv"))
marker<-list()
for(tr in c("A","B")){
 p<-file.path(root,"audits/G8_DESeq2_20261009",paste0("track_",tr))
 de<-read.csv(file.path(p,"deseq2_full_results.csv"))
 d<-merge(ann,de,by="gene_id",all.x=TRUE,sort=FALSE)
 d$track<-tr
 marker[[tr]]<-d
 write.csv(d,file.path(a,paste0("markers_track_",tr,"_v2.csv")),row.names=FALSE)
 pc<-read.csv(file.path(p,"pca_coordinates.csv"))
 extra<-read.csv(file.path(root,"audits/G9_validation_20261009",paste0("track_",tr,"_sample_qc.csv")))
 o<-merge(merge(pc,qc,by="run"),extra[,c("run","pca_distance_rank","distance_from_pca_center")],by="run")
 stopifnot(nrow(o)==nrow(pc))
 f<-c("salmon_mapping_pct","salmon_decoy_pct","mean_length_fastqc","fastqc_deduplicated_pct","gc_pct")
 q<-data.frame(metric=f,rho_PC1=vapply(f,function(k)cor(o[[k]],o$PC1,method="spearman"),numeric(1)),rho_PC2=vapply(f,function(k)cor(o[[k]],o$PC2,method="spearman"),numeric(1)),n=nrow(o))
 write.csv(q,file.path(a,paste0("pca_qc_correlations_track_",tr,"_v2.csv")),row.names=FALSE)
 cat("TRACK",tr,"QCCORR",paste(sprintf("%s=%.3f",q$metric,q$rho_PC1),collapse=";"),"\n")
 cat("TRACK",tr,"MARKERS",paste(sprintf("%s:LFC=%s;padj=%s",d$gene_name,round(d$log2FoldChange,2),signif(d$padj,2)),collapse=" | "),"\n")
 cat("TRACK",tr,"PCA_FARTHEST",paste(head(o$run[order(o$pca_distance_rank)],5),collapse=","),"\n")
}
stopifnot(nrow(marker$A)==7,nrow(marker$B)==7)
cat("G9_MARKER_PCA_PASS\n")